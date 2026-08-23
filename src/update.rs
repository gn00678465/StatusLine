//! Checks for available binary updates.

use std::fs;
use std::time::UNIX_EPOCH;

use serde_json::Value;

use crate::cachedir::{CacheDir, Clock};
use crate::oauth::HttpClient;
use crate::render::color::{DIM, RESET};

const CACHE_NAME: &str = "statusline-version-cache.json";
const CACHE_TTL_SECONDS: u64 = 86_400;
const LOCK_MAX_AGE_SECONDS: u64 = 30;
const RELEASE_URL: &str = "https://api.github.com/repos/gn00678465/StatusLine/releases/latest";
const REPOSITORY_URL: &str = "https://github.com/gn00678465/StatusLine";

pub struct UpdateChecker<H, C> {
    http: H,
    cache_dir: CacheDir,
    clock: C,
}

impl<H, C> UpdateChecker<H, C>
where
    H: HttpClient,
    C: Clock,
{
    pub fn new(http: H, cache_dir: CacheDir, clock: C) -> Self {
        Self {
            http,
            cache_dir,
            clock,
        }
    }

    pub fn check(&self) -> String {
        let cached_response = self.read_cached_response();
        if cached_response.is_some() && self.cache_is_fresh() {
            return render_update_line(cached_response.as_deref());
        }

        let refreshed_response = self.refresh();
        let response = refreshed_response
            .filter(|response| !response.is_empty())
            .or(cached_response);

        render_update_line(response.as_deref())
    }

    fn refresh(&self) -> Option<String> {
        if !self.cache_dir.is_safe() {
            return self.http.get_url(RELEASE_URL);
        }

        let lock = self
            .cache_dir
            .try_lock(CACHE_NAME, LOCK_MAX_AGE_SECONDS, &self.clock)?;
        let response = self.http.get_url(RELEASE_URL);
        if let Some(response) = response.as_deref().filter(|response| !response.is_empty()) {
            let _write_succeeded = self.cache_dir.atomic_write(CACHE_NAME, response.as_bytes());
        }
        drop(lock);

        response
    }

    fn read_cached_response(&self) -> Option<String> {
        let content = self.cache_dir.read(CACHE_NAME)?;

        String::from_utf8(content).ok()
    }

    fn cache_is_fresh(&self) -> bool {
        let cache_path = match self.cache_dir.path() {
            Some(cache_dir) => cache_dir.join(CACHE_NAME),
            None => return false,
        };
        let metadata = match fs::symlink_metadata(cache_path) {
            Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => metadata,
            Ok(_) | Err(_) => return false,
        };
        let modified_at = match metadata.modified().and_then(|time| {
            time.duration_since(UNIX_EPOCH)
                .map_err(std::io::Error::other)
        }) {
            Ok(duration) => duration.as_secs(),
            Err(_) => return false,
        };

        self.clock
            .now_epoch()
            .checked_sub(modified_at)
            .is_some_and(|age| age < CACHE_TTL_SECONDS)
    }
}

pub fn is_newer_version(candidate: &str, current: &str) -> bool {
    match (semver_components(candidate), semver_components(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn semver_components(version: &str) -> Option<[u64; 3]> {
    let version = match version.strip_prefix('v') {
        Some(version_without_prefix) => version_without_prefix,
        None => version,
    };
    let mut components = [0_u64; 3];

    for (index, component) in version.split('.').take(3).enumerate() {
        if !component.is_empty() {
            components[index] = component.parse::<u64>().ok()?;
        }
    }

    Some(components)
}

fn render_update_line(response: Option<&str>) -> String {
    let tag = match response.and_then(release_tag) {
        Some(tag) if is_newer_version(&tag, env!("CARGO_PKG_VERSION")) => tag,
        Some(_) | None => return String::new(),
    };

    format!("\n{DIM}Update available: {tag} → {REPOSITORY_URL}{RESET}")
}

fn release_tag(response: &str) -> Option<String> {
    let value: Value = serde_json::from_str(response).ok()?;
    let tag = value.get("tag_name")?.as_str()?;
    let tag: String = tag
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '+' | '-')
        })
        .collect();

    (!tag.is_empty()).then_some(tag)
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use tempfile::tempdir;

    use crate::cachedir::CacheDir;
    use crate::oauth::HttpClient;
    use crate::width::strip_ansi;

    use super::{is_newer_version, UpdateChecker};

    #[derive(Clone)]
    struct MockHttp {
        response: Option<String>,
        calls: Arc<AtomicUsize>,
    }

    impl HttpClient for MockHttp {
        fn get_usage(&self, _token: &str) -> Option<String> {
            None
        }

        fn get_url(&self, _url: &str) -> Option<String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.response.clone()
        }
    }

    struct FixedClock {
        now: u64,
    }

    impl crate::cachedir::Clock for FixedClock {
        fn now_epoch(&self) -> u64 {
            self.now
        }
    }

    #[test]
    fn compares_first_three_semver_components_with_shell_normalization() {
        assert!(is_newer_version("v1.2.4", "1.2.3"));
        assert!(is_newer_version("1.3", "1.2.99"));
        assert!(is_newer_version("1.2.1", "v1.2"));
        assert!(!is_newer_version("1.2.3", "v1.2.3"));
        assert!(!is_newer_version("v1.2.2", "1.2.3"));
    }

    #[test]
    fn reuses_a_fresh_twenty_four_hour_release_cache() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let checker = UpdateChecker::new(
            MockHttp {
                response: Some(r#"{"tag_name":"v999.0.0"}"#.to_owned()),
                calls: Arc::clone(&calls),
            },
            CacheDir::from_paths(None, Some(home.path())),
            FixedClock { now },
        );

        let first = checker.check();
        let second = checker.check();

        assert_eq!(calls.load(Ordering::Relaxed), 1);
        assert_eq!(first, second);
        assert_eq!(
            strip_ansi(&first),
            "\nUpdate available: v999.0.0 → https://github.com/gn00678465/StatusLine"
        );

        Ok(())
    }

    #[test]
    fn caches_a_non_empty_error_response_to_avoid_repeated_requests() -> Result<(), Box<dyn Error>>
    {
        let home = tempdir()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let checker = UpdateChecker::new(
            MockHttp {
                response: Some(r#"{"message":"API rate limit exceeded"}"#.to_owned()),
                calls: Arc::clone(&calls),
            },
            CacheDir::from_paths(None, Some(home.path())),
            FixedClock { now },
        );

        assert!(checker.check().is_empty());
        assert!(checker.check().is_empty());
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        Ok(())
    }

    #[test]
    fn removes_escape_characters_from_a_newer_release_tag() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let checker = UpdateChecker::new(
            MockHttp {
                response: Some(r#"{"tag_name":"v999.0.\u001b1"}"#.to_owned()),
                calls: Arc::new(AtomicUsize::new(0)),
            },
            CacheDir::from_paths(None, Some(home.path())),
            FixedClock { now },
        );

        let rendered = checker.check();

        assert_eq!(
            strip_ansi(&rendered),
            "\nUpdate available: v999.0.1 → https://github.com/gn00678465/StatusLine"
        );
        assert_eq!(rendered.matches('\u{001b}').count(), 2);

        Ok(())
    }

    #[test]
    fn returns_stale_data_when_another_process_holds_the_refresh_lock() -> Result<(), Box<dyn Error>>
    {
        let home = tempdir()?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        assert!(cache_dir.atomic_write(super::CACHE_NAME, br#"{"tag_name":"v999.0.0"}"#,));
        let cache_path = cache_dir
            .path()
            .ok_or("missing safe cache directory")?
            .join(super::CACHE_NAME);
        let cache_file = std::fs::File::open(cache_path)?;
        let stale_time = UNIX_EPOCH + Duration::from_secs(now.saturating_sub(86_400));
        cache_file.set_times(std::fs::FileTimes::new().set_modified(stale_time))?;
        let held_lock = cache_dir
            .try_lock(super::CACHE_NAME, 30, &FixedClock { now })
            .ok_or("failed to acquire update lock")?;
        let calls = Arc::new(AtomicUsize::new(0));
        let checker = UpdateChecker::new(
            MockHttp {
                response: Some(r#"{"tag_name":"v1000.0.0"}"#.to_owned()),
                calls: Arc::clone(&calls),
            },
            cache_dir,
            FixedClock { now },
        );

        let rendered = checker.check();

        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(
            strip_ansi(&rendered),
            "\nUpdate available: v999.0.0 → https://github.com/gn00678465/StatusLine"
        );
        drop(held_lock);

        Ok(())
    }
}
