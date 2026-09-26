//! Parses status-line configuration from environment variables and an
//! optional `~/.config/cc-statusline/config.toml` file.

use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::de::{Deserialize, Deserializer, Error as _};

use crate::render::meter::MeterStyle;
use crate::render::theme::{Layout, MeterTheme, Palette, Rgb, Theme, MAX_METER_WIDTH};

const DEFAULT_GIT_CACHE_TTL_SECONDS: u64 = 2;
const DEFAULT_COLUMNS: usize = 100;
/// Measured in Claude Code: it keeps 2 columns free on each side of the
/// status line, plus `statusLine.padding` on each side.
const CLAUDE_CODE_MARGIN_COLUMNS: usize = 4;
const MAX_PADDING: u8 = 20;
/// A 1 MiB main-thread stack overflows parsing `basic-toml` around nesting
/// depth 2500 (~5 KB file); this cap keeps every read far below that.
const MAX_CONFIG_FILE_BYTES: usize = 16_384;
/// A dedicated 16 MiB stack parses nesting depth 32768 (~64 KB) without
/// overflowing, leaving roughly 5x headroom over `MAX_CONFIG_FILE_BYTES`.
const PARSER_STACK_SIZE_BYTES: usize = 16 << 20;

/// New keys are validated while deserializing so a bad value takes the same
/// whole-file-ignore path as a TOML syntax error; `usage_style` and
/// `git_cache_ttl` keep their older lenient per-key fallback.
#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub(crate) struct FileConfig {
    usage_style: Option<String>,
    git_cache_ttl: Option<u64>,
    #[serde(deserialize_with = "padding")]
    padding: u8,
    layout: Layout,
    meter: FileMeter,
    colors: FileColors,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
struct FileMeter {
    #[serde(deserialize_with = "meter_width")]
    width: Option<usize>,
    #[serde(deserialize_with = "glyph")]
    filled: Option<String>,
    #[serde(deserialize_with = "glyph")]
    empty: Option<String>,
    show_percentage: Option<bool>,
    show_reset: Option<bool>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
struct FileColors {
    folder: Option<Rgb>,
    branch: Option<Rgb>,
    model: Option<Rgb>,
    tokens: Option<Rgb>,
    levels: Option<[Rgb; 4]>,
    #[serde(deserialize_with = "thresholds")]
    thresholds: Option<[u8; 3]>,
}

fn padding<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let padding = u8::deserialize(deserializer)?;
    if padding <= MAX_PADDING {
        Ok(padding)
    } else {
        Err(D::Error::custom(format!(
            "padding {padding} is outside 0..={MAX_PADDING}"
        )))
    }
}

fn meter_width<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<usize>, D::Error> {
    let width = usize::deserialize(deserializer)?;
    if (1..=MAX_METER_WIDTH).contains(&width) {
        Ok(Some(width))
    } else {
        Err(D::Error::custom(format!(
            "{width} is outside 1..={MAX_METER_WIDTH}"
        )))
    }
}

fn glyph<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<String>, D::Error> {
    let glyph = String::deserialize(deserializer)?;
    if glyph.is_empty() || glyph.chars().any(char::is_control) {
        Err(D::Error::custom(format!(
            "{glyph:?} must be non-empty without control characters"
        )))
    } else {
        Ok(Some(glyph))
    }
}

fn thresholds<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<[u8; 3]>, D::Error> {
    let [low, middle, high] = <[u8; 3]>::deserialize(deserializer)?;
    if low < middle && middle < high && high <= 100 {
        Ok(Some([low, middle, high]))
    } else {
        Err(D::Error::custom(format!(
            "[{low}, {middle}, {high}] must be strictly ascending and <= 100"
        )))
    }
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
    theme: Theme,
    git_cache_ttl_seconds: u64,
    /// Columns Claude Code shows before it cuts the line with `…`.
    width: usize,
}

impl Config {
    pub(crate) fn resolve(env: EnvValues, file: FileConfig) -> Self {
        let usage_style = non_empty(env.usage_style).or(file.usage_style);
        let git_cache_ttl =
            non_empty(env.git_cache_ttl).or_else(|| file.git_cache_ttl.map(|ttl| ttl.to_string()));
        let columns = non_empty(env.columns);
        let base = Self::from_values(
            usage_style.as_deref(),
            git_cache_ttl.as_deref(),
            columns.as_deref(),
            file.padding,
        );

        Self {
            theme: overlay_theme(base.theme, file.layout, file.meter, file.colors),
            ..base
        }
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
        padding: u8,
    ) -> Self {
        let margin = CLAUDE_CODE_MARGIN_COLUMNS + 2 * usize::from(padding);

        Self {
            theme: Theme::new(parse_usage_style(usage_style)),
            git_cache_ttl_seconds: parse_git_cache_ttl(git_cache_ttl),
            width: parse_columns(columns).saturating_sub(margin),
        }
    }

    pub(crate) fn theme(&self) -> &Theme {
        &self.theme
    }

    pub(crate) fn git_cache_ttl_seconds(&self) -> u64 {
        self.git_cache_ttl_seconds
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn parse_usage_style(value: Option<&str>) -> MeterStyle {
    match value {
        Some("dots") => MeterStyle::Dots,
        Some("bar") | Some(_) | None => MeterStyle::Bar,
    }
}

fn overlay_theme(defaults: Theme, layout: Layout, meter: FileMeter, colors: FileColors) -> Theme {
    Theme {
        layout,
        meter: MeterTheme {
            style: defaults.meter.style,
            width: meter.width.unwrap_or(defaults.meter.width),
            filled: meter.filled.unwrap_or(defaults.meter.filled),
            empty: meter.empty.unwrap_or(defaults.meter.empty),
            show_percentage: meter
                .show_percentage
                .unwrap_or(defaults.meter.show_percentage),
            show_reset: meter.show_reset.unwrap_or(defaults.meter.show_reset),
        },
        colors: Palette {
            folder: colors.folder.unwrap_or(defaults.colors.folder),
            branch: colors.branch.unwrap_or(defaults.colors.branch),
            model: colors.model.unwrap_or(defaults.colors.model),
            tokens: colors.tokens.unwrap_or(defaults.colors.tokens),
            levels: colors.levels.or(defaults.colors.levels),
            thresholds: colors.thresholds.unwrap_or(defaults.colors.thresholds),
        },
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

    use crate::render::meter::MeterStyle;
    use crate::render::theme::{Layout, MeterTheme, Palette, Rgb, Theme};

    use super::{config_file_path, read_config_file, Config, EnvValues, FileConfig};

    #[test]
    fn missing_config_file_yields_defaults_without_diagnostic() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("does-not-exist.toml");

        let (file, diagnostic) = read_config_file(&path);
        let config = Config::resolve(EnvValues::default(), file);

        assert_eq!(config.theme().meter.style, MeterStyle::Bar);
        assert_eq!(config.git_cache_ttl_seconds(), 2);
        assert_eq!(config.width(), 96);
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
                config.theme().meter.style,
                MeterStyle::Bar,
                "contents: {contents}"
            );
            assert_eq!(config.git_cache_ttl_seconds(), 2, "contents: {contents}");
            assert_eq!(config.width(), 96, "contents: {contents}");

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
    fn non_utf8_config_file_yields_defaults_with_diagnostic() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::write(&path, [0xFFu8, 0xFE, 0xFD])?;

        let (file, diagnostic) = read_config_file(&path);
        let config = Config::resolve(EnvValues::default(), file);

        assert_eq!(config.theme().meter.style, MeterStyle::Bar);
        assert_eq!(config.git_cache_ttl_seconds(), 2);
        assert_eq!(config.width(), 96);
        let diagnostic = diagnostic.ok_or("expected a diagnostic for non-UTF-8 content")?;
        assert!(
            diagnostic.contains(&path.display().to_string()),
            "diagnostic {diagnostic:?} should mention the path"
        );

        Ok(())
    }

    #[test]
    fn unreadable_config_path_yields_defaults_with_diagnostic() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::create_dir(&path)?;

        let (file, diagnostic) = read_config_file(&path);
        let config = Config::resolve(EnvValues::default(), file);

        assert_eq!(config.theme().meter.style, MeterStyle::Bar);
        assert_eq!(config.git_cache_ttl_seconds(), 2);
        assert_eq!(config.width(), 96);
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

        assert_eq!(config.theme().meter.style, MeterStyle::Bar);
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
            Config::resolve(EnvValues::default(), file)
                .theme()
                .meter
                .style,
            MeterStyle::Dots
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

        assert_eq!(
            Config::resolve(env, file).theme().meter.style,
            MeterStyle::Dots
        );
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
            Config::resolve(env_bar, file_dots).theme().meter.style,
            MeterStyle::Bar
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
            Config::resolve(env_dots, file_bar).theme().meter.style,
            MeterStyle::Dots
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
        assert_eq!(
            Config::resolve(env, file).theme().meter.style,
            MeterStyle::Dots
        );

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
            Config::resolve(EnvValues::default(), file)
                .theme()
                .meter
                .style,
            MeterStyle::Bar
        );
    }

    #[test]
    fn unknown_file_keys_are_ignored() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "future_key = true\nusage_style = \"dots\"\n")?;

        let (file, diagnostic) = read_config_file(&path);

        assert_eq!(
            Config::resolve(EnvValues::default(), file)
                .theme()
                .meter
                .style,
            MeterStyle::Dots
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

        assert_eq!(config.width(), 96);

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
        assert_eq!(configured.theme().meter.style, MeterStyle::Dots);
        assert_eq!(configured.git_cache_ttl_seconds(), 60);
        assert_eq!(configured.width(), 246);

        let fallback = Config::resolve(
            EnvValues {
                usage_style: Some("invalid".to_owned()),
                git_cache_ttl: Some("not-a-number".to_owned()),
                columns: Some("0".to_owned()),
            },
            file.clone(),
        );
        assert_eq!(fallback.theme().meter.style, MeterStyle::Bar);
        assert_eq!(fallback.git_cache_ttl_seconds(), 2);
        assert_eq!(fallback.width(), 96);

        let negative_ttl = Config::resolve(
            EnvValues {
                usage_style: None,
                git_cache_ttl: Some("-1".to_owned()),
                columns: Some("not-a-number".to_owned()),
            },
            file.clone(),
        );
        assert_eq!(negative_ttl.git_cache_ttl_seconds(), 2);
        assert_eq!(negative_ttl.width(), 96);

        let missing_columns = Config::resolve(EnvValues::default(), file);
        assert_eq!(missing_columns.width(), 96);
    }

    #[test]
    fn parses_usage_style_ttl_and_columns_with_clamps_and_fallbacks() {
        let configured = Config::from_values(Some("dots"), Some("99"), Some("250"), 0);

        assert_eq!(configured.theme().meter.style, MeterStyle::Dots);
        assert_eq!(configured.git_cache_ttl_seconds(), 60);
        assert_eq!(configured.width(), 246);

        let fallback = Config::from_values(Some("invalid"), Some("not-a-number"), Some("0"), 0);

        assert_eq!(fallback.theme().meter.style, MeterStyle::Bar);
        assert_eq!(fallback.git_cache_ttl_seconds(), 2);
        assert_eq!(fallback.width(), 96);

        let negative_ttl = Config::from_values(None, Some("-1"), Some("not-a-number"), 0);

        assert_eq!(negative_ttl.git_cache_ttl_seconds(), 2);
        assert_eq!(negative_ttl.width(), 96);

        let missing_columns = Config::from_values(None, None, None, 0);
        assert_eq!(missing_columns.width(), 96);
    }

    #[test]
    fn padding_reduces_the_usable_width_by_two_columns_per_unit() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        let width_with = |padding: &str, columns: &str| -> Result<usize, Box<dyn Error>> {
            std::fs::write(&path, padding)?;
            let (file, diagnostic) = read_config_file(&path);
            assert_eq!(diagnostic, None, "contents: {padding}");
            let env = EnvValues {
                columns: Some(columns.to_owned()),
                ..EnvValues::default()
            };

            Ok(Config::resolve(env, file).width())
        };

        assert_eq!(width_with("", "94")?, 90);
        assert_eq!(width_with("padding = 0", "94")?, 90);
        assert_eq!(width_with("padding = 1", "94")?, 88);
        assert_eq!(width_with("padding = 2", "94")?, 86);
        assert_eq!(width_with("padding = 20", "94")?, 50);
        assert_eq!(width_with("padding = 20", "40")?, 0);

        Ok(())
    }

    #[test]
    fn theme_keys_resolve_into_the_theme() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r##"
usage_style = "dots"
layout = "stacked"

[meter]
width = 20
filled = "#"
empty = "."
show_percentage = false
show_reset = false

[colors]
folder = "#010203"
branch = "#0A0B0C"
model = "#ffffff"
tokens = "#000000"
levels = ["#000001", "#000002", "#000003", "#000004"]
thresholds = [0, 1, 100]
"##,
        )?;

        let (file, diagnostic) = read_config_file(&path);
        let config = Config::resolve(EnvValues::default(), file);

        assert_eq!(diagnostic, None);
        assert_eq!(
            config.theme(),
            &Theme {
                layout: Layout::Stacked,
                meter: MeterTheme {
                    style: MeterStyle::Dots,
                    width: 20,
                    filled: "#".to_owned(),
                    empty: ".".to_owned(),
                    show_percentage: false,
                    show_reset: false,
                },
                colors: Palette {
                    folder: Rgb(1, 2, 3),
                    branch: Rgb(10, 11, 12),
                    model: Rgb(255, 255, 255),
                    tokens: Rgb(0, 0, 0),
                    levels: Some([Rgb(0, 0, 1), Rgb(0, 0, 2), Rgb(0, 0, 3), Rgb(0, 0, 4)]),
                    thresholds: [0, 1, 100],
                },
            }
        );

        Ok(())
    }

    #[test]
    fn absent_theme_keys_keep_style_defaults() -> Result<(), Box<dyn Error>> {
        let dir = tempdir()?;
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[meter]\nwidth = 1\n[colors]\nmodel = \"#010203\"\n")?;

        let (file, diagnostic) = read_config_file(&path);
        let env = EnvValues {
            usage_style: Some("dots".to_owned()),
            ..EnvValues::default()
        };
        let theme = Config::resolve(env, file).theme().clone();

        assert_eq!(diagnostic, None);
        assert_eq!(theme.layout, Layout::Auto);
        assert_eq!(theme.meter.width, 1);
        assert_eq!(
            (theme.meter.filled.as_str(), theme.meter.empty.as_str()),
            ("●", "○")
        );
        assert!(theme.meter.show_percentage && theme.meter.show_reset);
        assert_eq!(theme.colors.model.to_string(), "\u{1b}[38;2;1;2;3m");
        assert_eq!(theme.colors.folder.to_string(), "\u{1b}[38;2;100;220;255m");
        assert_eq!(theme.colors.levels, None);
        assert_eq!(theme.colors.thresholds, [50, 70, 90]);

        Ok(())
    }

    #[test]
    fn invalid_theme_keys_ignore_the_whole_file_with_a_diagnostic() -> Result<(), Box<dyn Error>> {
        let invalid_contents = [
            "padding = 21",
            "padding = -1",
            "padding = \"2\"",
            "layout = \"wide\"",
            "[meter]\nwidth = 0",
            "[meter]\nwidth = 21",
            "[meter]\nwidth = -1",
            "[meter]\nfilled = \"\"",
            "[meter]\nempty = \"\\t\"",
            "[meter]\nshow_reset = \"no\"",
            "[colors]\nfolder = \"#12345G\"",
            "[colors]\nbranch = \"#12345\"",
            "[colors]\nmodel = \"123456\"",
            "[colors]\nlevels = [\"#000000\", \"#000000\", \"#000000\"]",
            "[colors]\nthresholds = [70, 50, 90]",
            "[colors]\nthresholds = [50, 50, 90]",
            "[colors]\nthresholds = [50, 70, 101]",
        ];

        for contents in invalid_contents {
            let dir = tempdir()?;
            let path = dir.path().join("config.toml");
            std::fs::write(&path, format!("usage_style = \"dots\"\n{contents}\n"))?;

            let (file, diagnostic) = read_config_file(&path);
            let config = Config::resolve(EnvValues::default(), file);

            assert_eq!(
                config.theme().meter.style,
                MeterStyle::Bar,
                "contents: {contents}"
            );
            assert_eq!(config.theme().meter.width, 10, "contents: {contents}");
            let diagnostic = diagnostic
                .ok_or_else(|| format!("expected a diagnostic for contents: {contents}"))?;
            assert!(
                diagnostic.contains(&path.display().to_string()),
                "diagnostic {diagnostic:?} should mention the path for contents: {contents}"
            );
        }

        Ok(())
    }
}
