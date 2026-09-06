//! Parses status-line configuration from environment variables and an
//! optional `~/.config/cc-statusline/config.toml` file.

use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

const DEFAULT_GIT_CACHE_TTL_SECONDS: u64 = 2;
const DEFAULT_COLUMNS: usize = 100;
/// A 1 MiB main-thread stack overflows parsing `basic-toml` around nesting
/// depth 2500 (~5 KB file); this cap keeps every read far below that.
const MAX_CONFIG_FILE_BYTES: usize = 16_384;
/// A dedicated 16 MiB stack parses nesting depth 32768 (~64 KB) without
/// overflowing, leaving roughly 5x headroom over `MAX_CONFIG_FILE_BYTES`.
const PARSER_STACK_SIZE_BYTES: usize = 16 << 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UsageStyle {
    Bar,
    Dots,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub(crate) struct FileConfig {
    usage_style: Option<String>,
    git_cache_ttl: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct EnvValues {
    usage_style: Option<String>,
    git_cache_ttl: Option<String>,
    columns: Option<String>,
}

pub(crate) fn config_file_path(
    xdg_config_home: Option<&OsStr>,
    home: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(xdg_config_home) = xdg_config_home.filter(|value| !value.is_empty()) {
        return Some(
            Path::new(xdg_config_home)
                .join("cc-statusline")
                .join("config.toml"),
        );
    }

    home.map(|home| {
        home.join(".config")
            .join("cc-statusline")
            .join("config.toml")
    })
}

pub(crate) fn read_config_file(path: &Path) -> (FileConfig, Option<String>) {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (FileConfig::default(), None)
        }
        Err(error) => return (FileConfig::default(), Some(diagnostic(path, &error))),
    };

    let mut bytes = Vec::new();
    if let Err(error) = file
        .take(MAX_CONFIG_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
    {
        return (FileConfig::default(), Some(diagnostic(path, &error)));
    }
    if bytes.len() > MAX_CONFIG_FILE_BYTES {
        let message = format!("file exceeds the {MAX_CONFIG_FILE_BYTES}-byte limit");
        return (FileConfig::default(), Some(diagnostic(path, &message)));
    }
    let contents = match String::from_utf8(bytes) {
        Ok(contents) => contents,
        Err(error) => return (FileConfig::default(), Some(diagnostic(path, &error))),
    };

    match parse_on_dedicated_thread(contents) {
        Ok(file_config) => (file_config, None),
        Err(message) => (FileConfig::default(), Some(diagnostic(path, &message))),
    }
}

/// Parses on a dedicated large-stack thread so a deeply nested but
/// under-the-byte-cap file cannot overflow the caller's stack. A spawn
/// failure, a parser panic, or a parse error all collapse into one
/// diagnostic string — this never panics or unwraps.
fn parse_on_dedicated_thread(contents: String) -> Result<FileConfig, String> {
    let spawned = std::thread::Builder::new()
        .stack_size(PARSER_STACK_SIZE_BYTES)
        .spawn(move || basic_toml::from_str::<FileConfig>(&contents));

    match spawned {
        Ok(handle) => match handle.join() {
            Ok(Ok(file_config)) => Ok(file_config),
            Ok(Err(error)) => Err(error.to_string()),
            Err(_) => Err("config file parser thread panicked".to_owned()),
        },
        Err(error) => Err(error.to_string()),
    }
}

fn diagnostic(path: &Path, error: &dyn std::fmt::Display) -> String {
    format!(
        "cc-statusline: ignoring config file {}: {error}",
        path.display()
    )
}

#[derive(Debug)]
pub(crate) struct Config {
    usage_style: UsageStyle,
    git_cache_ttl_seconds: u64,
    columns: usize,
}

impl Config {
    pub(crate) fn resolve(env: EnvValues, file: FileConfig) -> Self {
        let usage_style = non_empty(env.usage_style).or(file.usage_style);
        let git_cache_ttl =
            non_empty(env.git_cache_ttl).or_else(|| file.git_cache_ttl.map(|ttl| ttl.to_string()));
        let columns = non_empty(env.columns);

        Self::from_values(
            usage_style.as_deref(),
            git_cache_ttl.as_deref(),
            columns.as_deref(),
        )
    }

    /// Reads env vars and the user config file, in that priority order.
    pub(crate) fn load() -> (Self, Option<String>) {
        let env = EnvValues {
            usage_style: std::env::var("STATUSLINE_USAGE_STYLE").ok(),
            git_cache_ttl: std::env::var("STATUSLINE_GIT_CACHE_TTL").ok(),
            columns: std::env::var("COLUMNS").ok(),
        };
        let path = config_file_path(
            std::env::var_os("XDG_CONFIG_HOME").as_deref(),
            crate::cachedir::home_directory().as_deref(),
        );
        let (file, diagnostic) = match path {
            Some(path) => read_config_file(&path),
            None => (FileConfig::default(), None),
        };

        (Self::resolve(env, file), diagnostic)
    }

    pub(crate) fn from_values(
        usage_style: Option<&str>,
        git_cache_ttl: Option<&str>,
        columns: Option<&str>,
    ) -> Self {
        Self {
            usage_style: parse_usage_style(usage_style),
            git_cache_ttl_seconds: parse_git_cache_ttl(git_cache_ttl),
            columns: parse_columns(columns),
        }
    }

    pub(crate) fn usage_style(&self) -> UsageStyle {
        self.usage_style
    }

    pub(crate) fn git_cache_ttl_seconds(&self) -> u64 {
        self.git_cache_ttl_seconds
    }

    pub(crate) fn columns(&self) -> usize {
        self.columns
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn parse_usage_style(value: Option<&str>) -> UsageStyle {
    match value {
        Some("dots") => UsageStyle::Dots,
        Some("bar") | Some(_) | None => UsageStyle::Bar,
    }
}

fn parse_git_cache_ttl(value: Option<&str>) -> u64 {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .map_or(DEFAULT_GIT_CACHE_TTL_SECONDS, |seconds| seconds.min(60))
}

fn parse_columns(value: Option<&str>) -> usize {
    match value.and_then(|value| value.parse::<usize>().ok()) {
        Some(columns) if columns > 0 => columns,
        Some(_) | None => DEFAULT_COLUMNS,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::OsStr;
    use std::path::{Path, PathBuf};

    use tempfile::tempdir;

    use super::{config_file_path, read_config_file, Config, EnvValues, FileConfig, UsageStyle};

    #[test]
    fn missing_config_file_yields_defaults_without_diagnostic() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("does-not-exist.toml");

        let (file, diagnostic) = read_config_file(&path);
        let config = Config::resolve(EnvValues::default(), file);

        assert_eq!(config.usage_style(), UsageStyle::Bar);
        assert_eq!(config.git_cache_ttl_seconds(), 2);
        assert_eq!(config.columns(), 100);
        assert_eq!(diagnostic, None);

        Ok(())
    }

    #[test]
    fn malformed_config_file_yields_defaults_with_diagnostic() -> Result<(), Box<dyn Error>> {
        let malformed_contents = [
            "usage_style = ",
            "usage_style = 1",
            "git_cache_ttl = -1",
            "git_cache_ttl = \"2\"",
        ];

        for contents in malformed_contents {
            let dir = tempdir()?;
            let path = dir.path().join("config.toml");
            std::fs::write(&path, contents)?;

            let (file, diagnostic) = read_config_file(&path);
            let config = Config::resolve(EnvValues::default(), file);

            assert_eq!(
                config.usage_style(),
                UsageStyle::Bar,
                "contents: {contents}"
            );
            assert_eq!(config.git_cache_ttl_seconds(), 2, "contents: {contents}");
            assert_eq!(config.columns(), 100, "contents: {contents}");

            let diagnostic = diagnostic
                .ok_or_else(|| format!("expected a diagnostic for contents: {contents}"))?;
            assert!(
                diagnostic.contains(&path.display().to_string()),
                "diagnostic {diagnostic:?} should mention the path for contents: {contents}"
            );
        }

        Ok(())
    }

    #[test]
    fn unreadable_config_path_yields_defaults_with_diagnostic() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::create_dir(&path)?;

        let (file, diagnostic) = read_config_file(&path);
        let config = Config::resolve(EnvValues::default(), file);

        assert_eq!(config.usage_style(), UsageStyle::Bar);
        assert_eq!(config.git_cache_ttl_seconds(), 2);
        assert_eq!(config.columns(), 100);
        assert!(diagnostic.is_some());

        Ok(())
    }

    #[test]
    fn oversized_config_file_yields_defaults_with_diagnostic() -> Result<(), Box<dyn Error>> {
        const LIMIT: usize = 16_384;
        let padded_contents = |total_len: usize| -> String {
            let mut contents = String::from("usage_style = \"dots\"\n");
            contents.push_str(&"#".repeat(total_len - contents.len()));

            contents
        };

        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::write(&path, padded_contents(LIMIT + 1))?;

        let (file, diagnostic) = read_config_file(&path);
        let config = Config::resolve(EnvValues::default(), file);

        assert_eq!(config.usage_style(), UsageStyle::Bar);
        assert_eq!(config.git_cache_ttl_seconds(), 2);
        let diagnostic =
            diagnostic.ok_or("expected a diagnostic for a file over the 16384-byte limit")?;
        assert!(
            diagnostic.contains(&path.display().to_string()),
            "diagnostic {diagnostic:?} should mention the path"
        );
        assert!(
            diagnostic.contains("16384"),
            "diagnostic {diagnostic:?} should mention the 16384-byte limit"
        );

        std::fs::write(&path, padded_contents(LIMIT))?;
        let (file, diagnostic) = read_config_file(&path);

        assert_eq!(
            Config::resolve(EnvValues::default(), file).usage_style(),
            UsageStyle::Dots
        );
        assert_eq!(diagnostic, None);

        // 16384 bytes of '#' plus a multi-byte char ("中", 3 bytes) = 16387
        // bytes, split across the byte cap mid-codepoint. The size check
        // must fire before UTF-8 decoding, so the diagnostic still names
        // the byte limit rather than reporting a decode error.
        let boundary_multibyte = format!("{}中", "#".repeat(LIMIT));
        std::fs::write(&path, &boundary_multibyte)?;
        let (_file, diagnostic) = read_config_file(&path);
        let diagnostic = diagnostic.ok_or(
            "expected a diagnostic for a multi-byte-boundary file over the 16384-byte limit",
        )?;
        assert!(
            diagnostic.contains("16384"),
            "diagnostic {diagnostic:?} should mention the 16384-byte limit, not a UTF-8 decode error"
        );

        Ok(())
    }

    #[test]
    fn config_path_prefers_xdg_config_home_then_home_dot_config_then_none() {
        let xdg = OsStr::new("/x");
        let home = Path::new("/h");

        assert_eq!(
            config_file_path(Some(xdg), Some(home)),
            Some(PathBuf::from("/x/cc-statusline/config.toml"))
        );
        assert_eq!(
            config_file_path(None, Some(home)),
            Some(PathBuf::from("/h/.config/cc-statusline/config.toml"))
        );
        assert_eq!(
            config_file_path(Some(OsStr::new("")), Some(home)),
            Some(PathBuf::from("/h/.config/cc-statusline/config.toml"))
        );
        assert_eq!(config_file_path(None, None), None);
    }

    #[test]
    fn file_usage_style_dots_applies_when_env_absent() {
        let env = EnvValues::default();
        let file = FileConfig {
            usage_style: Some("dots".to_owned()),
            ..FileConfig::default()
        };

        assert_eq!(Config::resolve(env, file).usage_style(), UsageStyle::Dots);
    }

    #[test]
    fn env_usage_style_overrides_file() {
        let env_bar = EnvValues {
            usage_style: Some("bar".to_owned()),
            ..EnvValues::default()
        };
        let file_dots = FileConfig {
            usage_style: Some("dots".to_owned()),
            ..FileConfig::default()
        };
        assert_eq!(
            Config::resolve(env_bar, file_dots).usage_style(),
            UsageStyle::Bar
        );

        let env_dots = EnvValues {
            usage_style: Some("dots".to_owned()),
            ..EnvValues::default()
        };
        let file_bar = FileConfig {
            usage_style: Some("bar".to_owned()),
            ..FileConfig::default()
        };
        assert_eq!(
            Config::resolve(env_dots, file_bar).usage_style(),
            UsageStyle::Dots
        );
    }

    #[test]
    fn empty_env_value_is_absent_so_file_applies() {
        let env = EnvValues {
            usage_style: Some(String::new()),
            ..EnvValues::default()
        };
        let file = FileConfig {
            usage_style: Some("dots".to_owned()),
            ..FileConfig::default()
        };
        assert_eq!(Config::resolve(env, file).usage_style(), UsageStyle::Dots);

        let env = EnvValues {
            git_cache_ttl: Some(String::new()),
            ..EnvValues::default()
        };
        let file = FileConfig {
            git_cache_ttl: Some(7),
            ..FileConfig::default()
        };
        assert_eq!(Config::resolve(env, file).git_cache_ttl_seconds(), 7);
    }

    #[test]
    fn file_git_cache_ttl_applies_and_clamps_to_60() {
        let resolve_with_ttl = |ttl: u64| {
            Config::resolve(
                EnvValues::default(),
                FileConfig {
                    git_cache_ttl: Some(ttl),
                    ..FileConfig::default()
                },
            )
            .git_cache_ttl_seconds()
        };

        assert_eq!(resolve_with_ttl(7), 7);
        assert_eq!(resolve_with_ttl(99), 60);
        assert_eq!(resolve_with_ttl(0), 0);
    }

    #[test]
    fn invalid_file_usage_style_falls_back_to_bar() {
        let file = FileConfig {
            usage_style: Some("foo".to_owned()),
            ..FileConfig::default()
        };

        assert_eq!(
            Config::resolve(EnvValues::default(), file).usage_style(),
            UsageStyle::Bar
        );
    }

    #[test]
    fn unknown_file_keys_are_ignored() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "future_key = true\nusage_style = \"dots\"\n")?;

        let (file, diagnostic) = read_config_file(&path);

        assert_eq!(
            Config::resolve(EnvValues::default(), file).usage_style(),
            UsageStyle::Dots
        );
        assert_eq!(diagnostic, None);

        Ok(())
    }

    #[test]
    fn file_cannot_set_columns() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "columns = 50\n")?;

        let (file, _diagnostic) = read_config_file(&path);
        let config = Config::resolve(EnvValues::default(), file);

        assert_eq!(config.columns(), 100);

        Ok(())
    }

    #[test]
    fn env_only_behaviour_is_unchanged_without_file() {
        let file = FileConfig::default();

        let configured = Config::resolve(
            EnvValues {
                usage_style: Some("dots".to_owned()),
                git_cache_ttl: Some("99".to_owned()),
                columns: Some("250".to_owned()),
            },
            file.clone(),
        );
        assert_eq!(configured.usage_style(), UsageStyle::Dots);
        assert_eq!(configured.git_cache_ttl_seconds(), 60);
        assert_eq!(configured.columns(), 250);

        let fallback = Config::resolve(
            EnvValues {
                usage_style: Some("invalid".to_owned()),
                git_cache_ttl: Some("not-a-number".to_owned()),
                columns: Some("0".to_owned()),
            },
            file.clone(),
        );
        assert_eq!(fallback.usage_style(), UsageStyle::Bar);
        assert_eq!(fallback.git_cache_ttl_seconds(), 2);
        assert_eq!(fallback.columns(), 100);

        let negative_ttl = Config::resolve(
            EnvValues {
                usage_style: None,
                git_cache_ttl: Some("-1".to_owned()),
                columns: Some("not-a-number".to_owned()),
            },
            file.clone(),
        );
        assert_eq!(negative_ttl.git_cache_ttl_seconds(), 2);
        assert_eq!(negative_ttl.columns(), 100);

        let missing_columns = Config::resolve(EnvValues::default(), file);
        assert_eq!(missing_columns.columns(), 100);
    }

    #[test]
    fn parses_usage_style_ttl_and_columns_with_clamps_and_fallbacks() {
        let configured = Config::from_values(Some("dots"), Some("99"), Some("250"));

        assert_eq!(configured.usage_style(), UsageStyle::Dots);
        assert_eq!(configured.git_cache_ttl_seconds(), 60);
        assert_eq!(configured.columns(), 250);

        let fallback = Config::from_values(Some("invalid"), Some("not-a-number"), Some("0"));

        assert_eq!(fallback.usage_style(), UsageStyle::Bar);
        assert_eq!(fallback.git_cache_ttl_seconds(), 2);
        assert_eq!(fallback.columns(), 100);

        let negative_ttl = Config::from_values(None, Some("-1"), Some("not-a-number"));

        assert_eq!(negative_ttl.git_cache_ttl_seconds(), 2);
        assert_eq!(negative_ttl.columns(), 100);

        let missing_columns = Config::from_values(None, None, None);
        assert_eq!(missing_columns.columns(), 100);
    }
}
