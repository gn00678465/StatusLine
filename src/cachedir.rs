//! Safely manages status-line cache directories and files.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub trait Clock {
    fn now_epoch(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_epoch(&self) -> u64 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs(),
            Err(_) => 0,
        }
    }
}

#[derive(Debug)]
pub enum CacheDir {
    Safe(PathBuf),
    Unsafe,
}

impl CacheDir {
    pub fn from_environment() -> Self {
        let runtime_dir = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        let home_dir = home_directory();

        Self::from_paths(runtime_dir.as_deref(), home_dir.as_deref())
    }

    pub fn from_paths(runtime_dir: Option<&Path>, home_dir: Option<&Path>) -> Self {
        if let Some(runtime_dir) = runtime_dir.filter(|path| is_safe_directory(path)) {
            return Self::with_statusline_directory(runtime_dir);
        }

        let home_dir = match home_dir.filter(|path| is_safe_directory(path)) {
            Some(home_dir) => home_dir,
            None => return Self::Unsafe,
        };
        let cache_dir = match ensure_safe_child_directory(home_dir, ".cache") {
            Some(cache_dir) => cache_dir,
            None => return Self::Unsafe,
        };

        Self::with_statusline_directory(&cache_dir)
    }

    pub fn is_safe(&self) -> bool {
        match self {
            Self::Safe(path) => is_safe_directory(path),
            Self::Unsafe => false,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Safe(path) if is_safe_directory(path) => Some(path.as_path()),
            Self::Unsafe => None,
            Self::Safe(_) => None,
        }
    }

    pub fn try_lock<C: Clock>(
        &self,
        name: &str,
        max_age_seconds: u64,
        clock: &C,
    ) -> Option<CacheLock> {
        let lock_path = self.entry_path(&format!("{name}.lock"))?;

        remove_stale_lock(&lock_path, max_age_seconds, clock);

        if create_private_directory(&lock_path).is_err() {
            return None;
        }
        if !is_safe_directory(&lock_path) {
            let _remove_result = fs::remove_dir(&lock_path);
            return None;
        }

        Some(CacheLock { path: lock_path })
    }

    pub fn atomic_write(&self, name: &str, contents: &[u8]) -> bool {
        let target_path = match self.entry_path(name) {
            Some(target_path) => target_path,
            None => return false,
        };
        let parent_dir = match target_path.parent() {
            Some(parent_dir) => parent_dir,
            None => return false,
        };
        let (mut temporary_file, temporary_path) = match create_private_temp_file(parent_dir) {
            Ok(file) => file,
            Err(_) => return false,
        };

        if temporary_file.write_all(contents).is_err() || temporary_file.sync_all().is_err() {
            drop(temporary_file);
            let _remove_result = fs::remove_file(&temporary_path);
            return false;
        }
        drop(temporary_file);

        if fs::rename(&temporary_path, &target_path).is_ok() {
            true
        } else {
            let _remove_result = fs::remove_file(&temporary_path);
            false
        }
    }

    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let entry_path = self.entry_path(name)?;
        let metadata = fs::symlink_metadata(&entry_path).ok()?;

        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return None;
        }

        fs::read(entry_path).ok()
    }

    fn with_statusline_directory(base_dir: &Path) -> Self {
        match ensure_safe_child_directory(base_dir, "StatusLine") {
            Some(cache_dir) => Self::Safe(cache_dir),
            None => Self::Unsafe,
        }
    }

    fn entry_path(&self, name: &str) -> Option<PathBuf> {
        if !is_valid_entry_name(name) {
            return None;
        }

        self.path().map(|cache_dir| cache_dir.join(name))
    }
}

pub struct CacheLock {
    path: PathBuf,
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        let _remove_result = fs::remove_dir(&self.path);
    }
}

fn create_private_temp_file(parent_dir: &Path) -> std::io::Result<(File, PathBuf)> {
    const MAX_ATTEMPTS: usize = 16;

    for _ in 0..MAX_ATTEMPTS {
        let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary_path = parent_dir.join(format!(
            ".cc-statusline-{}-{counter}.tmp",
            std::process::id()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;

            options.mode(0o600);
        }

        match options.open(&temporary_path) {
            Ok(file) => return Ok((file, temporary_path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique cache temporary file",
    ))
}

fn home_directory() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
    }

    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

fn ensure_safe_child_directory(parent: &Path, name: &str) -> Option<PathBuf> {
    let child = parent.join(name);

    match fs::symlink_metadata(&child) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match create_private_directory(&child) {
                Ok(()) => {}
                Err(create_error) if create_error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return None,
            }
        }
        Err(_) => return None,
    }

    is_safe_directory(&child).then_some(child)
}

fn create_private_directory(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;

        let mut builder = fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(path)
    }

    #[cfg(windows)]
    {
        fs::create_dir(path)
    }
}

fn remove_stale_lock<C: Clock>(lock_path: &Path, max_age_seconds: u64, clock: &C) {
    let metadata = match fs::symlink_metadata(lock_path) {
        Ok(metadata) => metadata,
        Err(_) => return,
    };

    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return;
    }
    let modified_at = match metadata.modified().and_then(|time| {
        time.duration_since(UNIX_EPOCH)
            .map_err(std::io::Error::other)
    }) {
        Ok(duration) => duration.as_secs(),
        Err(_) => return,
    };
    let age_seconds = match clock.now_epoch().checked_sub(modified_at) {
        Some(age_seconds) => age_seconds,
        None => return,
    };

    if age_seconds > max_age_seconds {
        let _remove_result = fs::remove_dir(lock_path);
    }
}

fn is_valid_entry_name(name: &str) -> bool {
    let mut components = Path::new(name).components();

    matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    )
}

fn is_safe_directory(path: &Path) -> bool {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return false,
    };

    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return false;
    }

    platform_directory_is_safe(&metadata)
}

#[cfg(unix)]
fn platform_directory_is_safe(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    metadata.uid() == current_user_id() && metadata.mode() & 0o022 == 0
}

#[cfg(windows)]
fn platform_directory_is_safe(_: &fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn current_user_id() -> u32 {
    // SAFETY: `geteuid` has no preconditions and does not access Rust-managed memory.
    unsafe { libc::geteuid() }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::time::{SystemTime, UNIX_EPOCH};

    use tempfile::tempdir;

    use super::CacheDir;

    #[cfg(unix)]
    use std::os::unix::fs::{symlink, PermissionsExt};

    #[test]
    fn creates_a_safe_home_cache_chain() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        let expected_cache_dir = home.path().join(".cache").join("StatusLine");

        assert!(cache_dir.is_safe());
        assert_eq!(cache_dir.path(), Some(expected_cache_dir.as_path()));

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            assert_eq!(
                std::fs::metadata(home.path().join(".cache"))?.mode() & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(&expected_cache_dir)?.mode() & 0o777,
                0o700
            );
        }

        Ok(())
    }

    struct FixedClock {
        now: u64,
    }

    impl super::Clock for FixedClock {
        fn now_epoch(&self) -> u64 {
            self.now
        }
    }

    #[test]
    fn lock_contention_allows_only_one_holder() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        let clock = FixedClock { now: 100 };

        let first_lock = cache_dir.try_lock("git-status", 5, &clock);

        assert!(first_lock.is_some());
        assert!(cache_dir.try_lock("git-status", 5, &clock).is_none());

        drop(first_lock);

        assert!(cache_dir.try_lock("git-status", 5, &clock).is_some());

        Ok(())
    }

    #[test]
    fn clears_a_stale_lock_before_acquiring_it() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + 10;
        let clock = FixedClock { now };
        let stale_lock = cache_dir
            .try_lock("git-status", 5, &clock)
            .ok_or_else(|| std::io::Error::other("initial lock should be acquired"))?;

        std::mem::forget(stale_lock);

        assert!(cache_dir.try_lock("git-status", 5, &clock).is_some());

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_group_writable_runtime_directory() -> Result<(), Box<dyn Error>> {
        let runtime_dir = tempdir()?;
        std::fs::set_permissions(runtime_dir.path(), std::fs::Permissions::from_mode(0o770))?;

        let cache_dir = CacheDir::from_paths(Some(runtime_dir.path()), None);

        assert!(!cache_dir.is_safe());
        assert_eq!(cache_dir.path(), None);

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_group_writable_directory_at_any_home_cache_layer() -> Result<(), Box<dyn Error>> {
        let group_writable_home = tempdir()?;
        std::fs::set_permissions(
            group_writable_home.path(),
            std::fs::Permissions::from_mode(0o770),
        )?;
        assert!(!CacheDir::from_paths(None, Some(group_writable_home.path())).is_safe());

        let group_writable_cache_home = tempdir()?;
        let group_writable_cache = group_writable_cache_home.path().join(".cache");
        std::fs::create_dir(&group_writable_cache)?;
        std::fs::set_permissions(
            &group_writable_cache,
            std::fs::Permissions::from_mode(0o770),
        )?;
        assert!(!CacheDir::from_paths(None, Some(group_writable_cache_home.path())).is_safe());

        let group_writable_leaf_home = tempdir()?;
        let safe_cache = group_writable_leaf_home.path().join(".cache");
        let group_writable_leaf = safe_cache.join("StatusLine");
        std::fs::create_dir(&safe_cache)?;
        std::fs::set_permissions(&safe_cache, std::fs::Permissions::from_mode(0o700))?;
        std::fs::create_dir(&group_writable_leaf)?;
        std::fs::set_permissions(&group_writable_leaf, std::fs::Permissions::from_mode(0o770))?;
        assert!(!CacheDir::from_paths(None, Some(group_writable_leaf_home.path())).is_safe());

        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlink_in_the_home_cache_chain() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let target = tempdir()?;
        symlink(target.path(), home.path().join(".cache"))?;

        let cache_dir = CacheDir::from_paths(None, Some(home.path()));

        assert!(!cache_dir.is_safe());
        assert_eq!(cache_dir.path(), None);

        Ok(())
    }

    #[test]
    fn atomically_replaces_cache_files_with_owner_only_permissions() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        let cache_path = cache_dir
            .path()
            .ok_or_else(|| std::io::Error::other("cache directory should be safe"))?
            .join("usage.json");

        assert!(cache_dir.atomic_write("usage.json", b"first"));
        assert_eq!(std::fs::read(&cache_path)?, b"first");
        assert_eq!(cache_dir.read("usage.json"), Some(b"first".to_vec()));

        assert!(cache_dir.atomic_write("usage.json", b"replacement"));
        assert_eq!(std::fs::read(&cache_path)?, b"replacement");
        assert_eq!(cache_dir.read("usage.json"), Some(b"replacement".to_vec()));

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            assert_eq!(std::fs::metadata(&cache_path)?.mode() & 0o777, 0o600);
        }

        Ok(())
    }

    #[test]
    fn unsafe_cache_operations_are_noops() {
        let cache_dir = CacheDir::Unsafe;
        let clock = FixedClock { now: 100 };

        assert_eq!(cache_dir.read("usage.json"), None);
        assert!(!cache_dir.atomic_write("usage.json", b"content"));
        assert!(cache_dir.try_lock("usage", 5, &clock).is_none());
    }
}
