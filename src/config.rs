//! Parses status-line configuration from environment variables.

const DEFAULT_GIT_CACHE_TTL_SECONDS: u64 = 2;
const DEFAULT_COLUMNS: usize = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UsageStyle {
    Bar,
    Dots,
}

#[derive(Debug)]
pub(crate) struct Config {
    usage_style: UsageStyle,
    git_cache_ttl_seconds: u64,
    columns: usize,
}

impl Config {
    pub(crate) fn from_env() -> Self {
        let usage_style = std::env::var("STATUSLINE_USAGE_STYLE").ok();
        let git_cache_ttl = std::env::var("STATUSLINE_GIT_CACHE_TTL").ok();
        let columns = std::env::var("COLUMNS").ok();

        Self::from_values(
            usage_style.as_deref(),
            git_cache_ttl.as_deref(),
            columns.as_deref(),
        )
    }

    fn from_values(
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
    use super::{Config, UsageStyle};

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
