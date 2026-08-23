//! Collects and caches Git workspace status.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use crate::cachedir::{CacheDir, Clock};

const LOCK_MAX_AGE_SECONDS: u64 = 5;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitStatus {
    pub is_repository: bool,
    pub branch: Option<String>,
    pub staged: u32,
    pub unstaged: u32,
    pub conflicted: u32,
}

pub fn parse_porcelain(output: &str) -> GitStatus {
    let mut status = GitStatus::default();

    for line in output.lines() {
        if let Some(branch) = line.strip_prefix("# branch.head ") {
            let branch = match branch {
                "(detached)" => "detached".to_owned(),
                branch => sanitize_branch(branch),
            };
            status.branch = (!branch.is_empty()).then_some(branch);
            continue;
        }

        let bytes = line.as_bytes();
        match bytes {
            [b'1' | b'2', b' ', x, y, ..] => {
                if *x != b'.' {
                    status.staged = status.staged.saturating_add(1);
                }
                if *y != b'.' {
                    status.unstaged = status.unstaged.saturating_add(1);
                }
            }
            [b'u', b' ', ..] => {
                status.conflicted = status.conflicted.saturating_add(1);
            }
            _ => {}
        }
    }

    status.is_repository = status.branch.is_some();
    status
}

pub enum GitRunResult {
    Completed { stdout: String, success: bool },
    Failed,
    TimedOut,
}

pub trait GitRunner: Send + Sync + 'static {
    fn run_status(&self, repository: &Path) -> GitRunResult;
}

pub struct CommandGitRunner;

impl GitRunner for CommandGitRunner {
    fn run_status(&self, repository: &Path) -> GitRunResult {
        match Command::new("git")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(repository)
            .arg("status")
            .arg("--porcelain=v2")
            .arg("--branch")
            .arg("--no-ahead-behind")
            .arg("--untracked-files=no")
            .arg("--ignore-submodules=dirty")
            .arg("--no-renames")
            .output()
        {
            Ok(output) => GitRunResult::Completed {
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                success: output.status.success(),
            },
            Err(_) => GitRunResult::Failed,
        }
    }
}

pub struct GitStatusCollector<R, C> {
    runner: Arc<R>,
    cache_dir: CacheDir,
    clock: C,
    timeout: Duration,
}

impl<R, C> GitStatusCollector<R, C>
where
    R: GitRunner,
    C: Clock,
{
    pub fn new(runner: R, cache_dir: CacheDir, clock: C) -> Self {
        Self::with_timeout(runner, cache_dir, clock, Duration::from_secs(1))
    }

    pub fn collect(&self, repository: &Path, session_id: &str, ttl_seconds: u64) -> GitStatus {
        if ttl_seconds == 0 {
            return self.refresh(repository, None).status;
        }

        let cache_name = cache_entry_name(session_id);
        let cached_status = self.read_cached_status(&cache_name, repository);
        if let Some(cached_status) = cached_status.as_ref() {
            if self.cache_is_fresh(cached_status.timestamp, ttl_seconds) {
                return cached_status.status.clone();
            }
        }

        let stale_status = cached_status.and_then(|cached_status| {
            cached_status
                .status
                .is_repository
                .then_some(cached_status.status)
        });
        let lock = self
            .cache_dir
            .try_lock(&cache_name, LOCK_MAX_AGE_SECONDS, &self.clock);
        if lock.is_none() {
            if let Some(stale_status) = stale_status.as_ref() {
                return stale_status.clone();
            }
        }

        let refresh_result = self.refresh(repository, stale_status);
        if refresh_result.cacheable {
            self.write_cached_status(&cache_name, repository, &refresh_result.status);
        }
        drop(lock);

        refresh_result.status
    }

    fn with_timeout(runner: R, cache_dir: CacheDir, clock: C, timeout: Duration) -> Self {
        Self {
            runner: Arc::new(runner),
            cache_dir,
            clock,
            timeout,
        }
    }

    fn refresh(&self, repository: &Path, stale_status: Option<GitStatus>) -> RefreshResult {
        match self.run_with_timeout(repository) {
            GitRunResult::Completed {
                stdout,
                success: true,
            } => {
                let parsed_status = parse_porcelain(&stdout);
                let status = if parsed_status.is_repository {
                    parsed_status
                } else {
                    match stale_status {
                        Some(stale_status) => stale_status,
                        None => parsed_status,
                    }
                };

                RefreshResult {
                    status,
                    cacheable: true,
                }
            }
            GitRunResult::Completed { success: false, .. } | GitRunResult::Failed => {
                RefreshResult {
                    status: GitStatus::default(),
                    cacheable: true,
                }
            }
            GitRunResult::TimedOut => RefreshResult {
                status: stale_status.unwrap_or_default(),
                cacheable: false,
            },
        }
    }

    fn run_with_timeout(&self, repository: &Path) -> GitRunResult {
        let (sender, receiver) = mpsc::sync_channel(1);
        let runner = Arc::clone(&self.runner);
        let repository = PathBuf::from(repository);

        let _thread = thread::spawn(move || {
            let result = runner.run_status(&repository);
            let _send_result = sender.send(result);
        });

        match receiver.recv_timeout(self.timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => GitRunResult::TimedOut,
            Err(mpsc::RecvTimeoutError::Disconnected) => GitRunResult::Failed,
        }
    }

    fn read_cached_status(&self, cache_name: &str, repository: &Path) -> Option<CachedGitStatus> {
        let content = self.cache_dir.read(cache_name)?;
        decode_cached_status(&content, repository)
    }

    fn cache_is_fresh(&self, timestamp: u64, ttl_seconds: u64) -> bool {
        match self.clock.now_epoch().checked_sub(timestamp) {
            Some(age_seconds) => age_seconds < ttl_seconds,
            None => false,
        }
    }

    fn write_cached_status(&self, cache_name: &str, repository: &Path, status: &GitStatus) {
        let content = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n",
            self.clock.now_epoch(),
            repository_identity(repository),
            u8::from(status.is_repository),
            status.branch.as_deref().unwrap_or_default(),
            status.staged,
            status.unstaged,
            status.conflicted,
        );
        let _write_succeeded = self.cache_dir.atomic_write(cache_name, content.as_bytes());
    }
}

struct CachedGitStatus {
    timestamp: u64,
    status: GitStatus,
}

struct RefreshResult {
    status: GitStatus,
    cacheable: bool,
}

fn cache_entry_name(session_id: &str) -> String {
    let session_key: String = session_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '_' | '-') {
                character
            } else {
                '_'
            }
        })
        .take(32)
        .collect();
    let session_key = if session_key.is_empty() {
        "default"
    } else {
        session_key.as_str()
    };

    format!("git-status-{session_key}.cache")
}

fn decode_cached_status(content: &[u8], repository: &Path) -> Option<CachedGitStatus> {
    let content = std::str::from_utf8(content).ok()?;
    let mut lines = content.lines();
    let timestamp = lines.next()?.parse::<u64>().ok()?;
    let cached_repository = lines.next()?;
    let is_repository = match lines.next()? {
        "0" => false,
        "1" => true,
        _ => return None,
    };
    let branch = lines.next()?;
    let staged = lines.next()?.parse::<u32>().ok()?;
    let unstaged = lines.next()?.parse::<u32>().ok()?;
    let conflicted = lines.next()?.parse::<u32>().ok()?;

    if lines.next().is_some() || cached_repository != repository_identity(repository) {
        return None;
    }

    let branch = sanitize_branch(branch);
    let branch = if is_repository {
        (!branch.is_empty()).then_some(branch)?
    } else if branch.is_empty() {
        String::new()
    } else {
        return None;
    };

    Some(CachedGitStatus {
        timestamp,
        status: GitStatus {
            is_repository,
            branch: is_repository.then_some(branch),
            staged,
            unstaged,
            conflicted,
        },
    })
}

fn repository_identity(repository: &Path) -> String {
    repository.to_string_lossy().into_owned()
}

fn sanitize_branch(branch: &str) -> String {
    branch
        .chars()
        .filter(|character| {
            !matches!(
                character,
                '\u{0000}'..='\u{001F}'
                    | '\u{007F}'
                    | '\u{200B}'..='\u{200F}'
                    | '\u{202A}'..='\u{202E}'
                    | '\u{2066}'..='\u{2069}'
                    | '\u{FEFF}'
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::error::Error;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;

    use tempfile::tempdir;

    use crate::cachedir::{CacheDir, Clock};

    use super::{
        parse_porcelain, GitRunResult, GitRunner, GitStatus, GitStatusCollector,
        LOCK_MAX_AGE_SECONDS,
    };

    #[test]
    fn parses_mixed_porcelain_v2_status() {
        let status = parse_porcelain(
            "# branch.oid 0123456789abcdef\n\
             # branch.head feature/cache\n\
             1 M. N... 100644 100644 100644 abc def src/staged.rs\n\
             2 .M N... 100644 100644 100644 abc def R100 src/old.rs\tsrc/new.rs\n\
             u UU N... 100644 100644 100644 100644 abc def ghi src/conflict.rs\n",
        );

        assert!(status.is_repository);
        assert_eq!(status.branch.as_deref(), Some("feature/cache"));
        assert_eq!(status.staged, 1);
        assert_eq!(status.unstaged, 1);
        assert_eq!(status.conflicted, 1);
    }

    struct FixedClock {
        now: u64,
    }

    impl Clock for FixedClock {
        fn now_epoch(&self) -> u64 {
            self.now
        }
    }

    #[derive(Clone)]
    struct SharedClock {
        now: Arc<AtomicU64>,
    }

    impl Clock for SharedClock {
        fn now_epoch(&self) -> u64 {
            self.now.load(Ordering::Relaxed)
        }
    }

    struct CountingRunner {
        calls: Arc<AtomicUsize>,
        output: String,
    }

    impl GitRunner for CountingRunner {
        fn run_status(&self, _: &Path) -> GitRunResult {
            self.calls.fetch_add(1, Ordering::Relaxed);
            GitRunResult::Completed {
                stdout: self.output.clone(),
                success: true,
            }
        }
    }

    struct ScriptedRunner {
        calls: Arc<AtomicUsize>,
        responses: Mutex<VecDeque<GitRunResult>>,
    }

    impl GitRunner for ScriptedRunner {
        fn run_status(&self, _: &Path) -> GitRunResult {
            self.calls.fetch_add(1, Ordering::Relaxed);

            match self.responses.lock() {
                Ok(mut responses) => match responses.pop_front() {
                    Some(response) => response,
                    None => GitRunResult::Failed,
                },
                Err(_) => GitRunResult::Failed,
            }
        }
    }

    struct SlowRunner {
        calls: Arc<AtomicUsize>,
        delay: Duration,
    }

    impl GitRunner for SlowRunner {
        fn run_status(&self, _: &Path) -> GitRunResult {
            self.calls.fetch_add(1, Ordering::Relaxed);
            thread::sleep(self.delay);

            successful_status("late-result")
        }
    }

    fn successful_status(branch: &str) -> GitRunResult {
        GitRunResult::Completed {
            stdout: format!("# branch.head {branch}\n"),
            success: true,
        }
    }

    #[test]
    fn returns_a_matching_fresh_session_cache_without_running_git() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        let calls = Arc::new(AtomicUsize::new(0));
        let collector = GitStatusCollector::new(
            CountingRunner {
                calls: Arc::clone(&calls),
                output: "# branch.head main\n".to_owned(),
            },
            cache_dir,
            FixedClock { now: 100 },
        );

        let first = collector.collect(Path::new("/repo"), "session", 2);
        let second = collector.collect(Path::new("/repo"), "session", 2);

        assert_eq!(first, second);
        assert_eq!(first.branch.as_deref(), Some("main"));
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        Ok(())
    }

    #[test]
    fn caches_non_repository_results_within_ttl() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let collector = GitStatusCollector::new(
            ScriptedRunner {
                calls: Arc::clone(&calls),
                responses: Mutex::new(VecDeque::from([GitRunResult::Failed])),
            },
            CacheDir::from_paths(None, Some(home.path())),
            FixedClock { now: 100 },
        );

        let first = collector.collect(Path::new("/not-a-repository"), "session", 2);
        let second = collector.collect(Path::new("/not-a-repository"), "session", 2);

        assert_eq!(first, GitStatus::default());
        assert_eq!(second, GitStatus::default());
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        Ok(())
    }

    #[test]
    fn does_not_use_a_negative_cache_entry_as_stale() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        assert!(cache_dir.atomic_write(
            "git-status-session.cache",
            b"100\n/not-a-repository\n0\n\n0\n0\n0\n",
        ));
        let held_lock = cache_dir.try_lock(
            "git-status-session.cache",
            LOCK_MAX_AGE_SECONDS,
            &FixedClock { now: 102 },
        );
        assert!(held_lock.is_some());

        let calls = Arc::new(AtomicUsize::new(0));
        let collector = GitStatusCollector::new(
            ScriptedRunner {
                calls: Arc::clone(&calls),
                responses: Mutex::new(VecDeque::from([successful_status("recovered")])),
            },
            CacheDir::from_paths(None, Some(home.path())),
            FixedClock { now: 102 },
        );

        let status = collector.collect(Path::new("/not-a-repository"), "session", 2);

        assert_eq!(status.branch.as_deref(), Some("recovered"));
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        drop(held_lock);

        Ok(())
    }

    #[test]
    fn returns_stale_cache_when_successful_git_output_has_no_branch() -> Result<(), Box<dyn Error>>
    {
        let home = tempdir()?;
        let now = Arc::new(AtomicU64::new(100));
        let clock = SharedClock {
            now: Arc::clone(&now),
        };
        let warm_collector = GitStatusCollector::new(
            CountingRunner {
                calls: Arc::new(AtomicUsize::new(0)),
                output: "# branch.head main\n".to_owned(),
            },
            CacheDir::from_paths(None, Some(home.path())),
            clock.clone(),
        );
        let cached_status = warm_collector.collect(Path::new("/repo"), "session", 2);
        now.store(102, Ordering::Relaxed);

        let calls = Arc::new(AtomicUsize::new(0));
        let collector = GitStatusCollector::new(
            ScriptedRunner {
                calls: Arc::clone(&calls),
                responses: Mutex::new(VecDeque::from([GitRunResult::Completed {
                    stdout: String::new(),
                    success: true,
                }])),
            },
            CacheDir::from_paths(None, Some(home.path())),
            SharedClock { now },
        );

        let fallback = collector.collect(Path::new("/repo"), "session", 2);

        assert_eq!(fallback, cached_status);
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        Ok(())
    }

    #[test]
    fn parses_clean_staged_conflicted_detached_and_empty_repositories() {
        let clean = parse_porcelain("# branch.head main\n");
        assert_eq!(
            clean,
            GitStatus {
                is_repository: true,
                branch: Some("main".to_owned()),
                staged: 0,
                unstaged: 0,
                conflicted: 0,
            }
        );

        let staged = parse_porcelain("# branch.head main\n1 A. N... metadata\n");
        assert_eq!(staged.staged, 1);
        assert_eq!(staged.unstaged, 0);

        let conflicted = parse_porcelain("# branch.head main\nu UU N... metadata\n");
        assert_eq!(conflicted.conflicted, 1);

        let detached = parse_porcelain("# branch.head (detached)\n");
        assert_eq!(detached.branch.as_deref(), Some("detached"));

        let empty = parse_porcelain("# branch.head (initial)\n");
        assert_eq!(empty.branch.as_deref(), Some("(initial)"));
        assert!(empty.is_repository);
    }

    #[test]
    fn refreshes_an_expired_cache() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        let calls = Arc::new(AtomicUsize::new(0));
        let now = Arc::new(AtomicU64::new(100));
        let clock = SharedClock {
            now: Arc::clone(&now),
        };
        let collector = GitStatusCollector::new(
            ScriptedRunner {
                calls: Arc::clone(&calls),
                responses: Mutex::new(VecDeque::from([
                    successful_status("main"),
                    successful_status("next"),
                ])),
            },
            cache_dir,
            clock,
        );

        let first = collector.collect(Path::new("/repo"), "session", 2);
        now.store(102, Ordering::Relaxed);
        let refreshed = collector.collect(Path::new("/repo"), "session", 2);

        assert_eq!(first.branch.as_deref(), Some("main"));
        assert_eq!(refreshed.branch.as_deref(), Some("next"));
        assert_eq!(calls.load(Ordering::Relaxed), 2);

        Ok(())
    }

    #[test]
    fn refreshes_when_the_cached_repository_does_not_match() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        let calls = Arc::new(AtomicUsize::new(0));
        let collector = GitStatusCollector::new(
            ScriptedRunner {
                calls: Arc::clone(&calls),
                responses: Mutex::new(VecDeque::from([
                    successful_status("one"),
                    successful_status("two"),
                ])),
            },
            cache_dir,
            FixedClock { now: 100 },
        );

        let first = collector.collect(Path::new("/repo-one"), "session", 2);
        let second = collector.collect(Path::new("/repo-two"), "session", 2);

        assert_eq!(first.branch.as_deref(), Some("one"));
        assert_eq!(second.branch.as_deref(), Some("two"));
        assert_eq!(calls.load(Ordering::Relaxed), 2);

        Ok(())
    }

    #[test]
    fn disables_cache_when_ttl_is_zero() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        let calls = Arc::new(AtomicUsize::new(0));
        let collector = GitStatusCollector::new(
            ScriptedRunner {
                calls: Arc::clone(&calls),
                responses: Mutex::new(VecDeque::from([
                    successful_status("one"),
                    successful_status("two"),
                ])),
            },
            cache_dir,
            FixedClock { now: 100 },
        );

        let first = collector.collect(Path::new("/repo"), "session", 0);
        let second = collector.collect(Path::new("/repo"), "session", 0);

        assert_eq!(first.branch.as_deref(), Some("one"));
        assert_eq!(second.branch.as_deref(), Some("two"));
        assert_eq!(calls.load(Ordering::Relaxed), 2);

        Ok(())
    }

    #[test]
    fn returns_stale_cache_when_the_git_command_times_out() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let now = Arc::new(AtomicU64::new(100));
        let clock = SharedClock {
            now: Arc::clone(&now),
        };
        let warm_collector = GitStatusCollector::new(
            CountingRunner {
                calls: Arc::new(AtomicUsize::new(0)),
                output: "# branch.head main\n".to_owned(),
            },
            CacheDir::from_paths(None, Some(home.path())),
            clock.clone(),
        );
        let cached_status = warm_collector.collect(Path::new("/repo"), "session", 2);
        now.store(102, Ordering::Relaxed);

        let calls = Arc::new(AtomicUsize::new(0));
        let timed_collector = GitStatusCollector::with_timeout(
            SlowRunner {
                calls: Arc::clone(&calls),
                delay: Duration::from_millis(20),
            },
            CacheDir::from_paths(None, Some(home.path())),
            clock,
            Duration::from_millis(1),
        );
        let fallback = timed_collector.collect(Path::new("/repo"), "session", 2);

        assert_eq!(fallback, cached_status);
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let refreshed_collector = GitStatusCollector::new(
            CountingRunner {
                calls: Arc::clone(&refresh_calls),
                output: "# branch.head refreshed\n".to_owned(),
            },
            CacheDir::from_paths(None, Some(home.path())),
            SharedClock { now },
        );
        let refreshed = refreshed_collector.collect(Path::new("/repo"), "session", 2);

        assert_eq!(refreshed.branch.as_deref(), Some("refreshed"));
        assert_eq!(refresh_calls.load(Ordering::Relaxed), 1);

        Ok(())
    }
}
