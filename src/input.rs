//! Parses and sanitizes the status-line JSON input.

use serde::{Deserialize, Deserializer};

const DEFAULT_MODEL_NAME: &str = "Claude";
const DEFAULT_CONTEXT_WINDOW_SIZE: u64 = 200_000;

#[derive(Debug, Default, Deserialize)]
pub(crate) struct StatusInput {
    #[serde(default)]
    model: Option<Model>,
    #[serde(default)]
    effort: Option<Effort>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    cwd: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    session_id: Option<String>,
    #[serde(default)]
    context_window: Option<ContextWindow>,
    #[serde(default)]
    rate_limits: Option<RateLimits>,
}

#[derive(Debug, Default, Deserialize)]
struct Model {
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    display_name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Effort {
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    level: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ContextWindow {
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    context_window_size: Option<u64>,
    #[serde(default)]
    current_usage: Option<CurrentUsage>,
    #[serde(default, deserialize_with = "deserialize_optional_f64")]
    used_percentage: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct CurrentUsage {
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    cache_creation_input_tokens: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    cache_read_input_tokens: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct RateLimits {
    #[serde(default)]
    five_hour: Option<RateLimit>,
    #[serde(default)]
    seven_day: Option<RateLimit>,
}

#[derive(Debug, Default, Deserialize)]
struct RateLimit {
    #[serde(default, deserialize_with = "deserialize_optional_f64")]
    used_percentage: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    resets_at: Option<u64>,
}

impl StatusInput {
    pub(crate) fn model_display_name(&self) -> &str {
        match self
            .model
            .as_ref()
            .and_then(|model| model.display_name.as_deref())
        {
            Some(display_name) => display_name,
            None => DEFAULT_MODEL_NAME,
        }
    }

    pub(crate) fn context_window_size(&self) -> u64 {
        match self
            .context_window
            .as_ref()
            .and_then(|context_window| context_window.context_window_size)
        {
            Some(context_window_size) if context_window_size > 0 => context_window_size,
            Some(_) | None => DEFAULT_CONTEXT_WINDOW_SIZE,
        }
    }

    pub(crate) fn effort_level(&self) -> Option<&str> {
        self.effort
            .as_ref()
            .and_then(|effort| effort.level.as_deref())
    }

    pub(crate) fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    pub(crate) fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub(crate) fn input_tokens(&self) -> u64 {
        self.current_usage()
            .and_then(|usage| usage.input_tokens)
            .unwrap_or(0)
    }

    pub(crate) fn cache_creation_input_tokens(&self) -> u64 {
        self.current_usage()
            .and_then(|usage| usage.cache_creation_input_tokens)
            .unwrap_or(0)
    }

    pub(crate) fn cache_read_input_tokens(&self) -> u64 {
        self.current_usage()
            .and_then(|usage| usage.cache_read_input_tokens)
            .unwrap_or(0)
    }

    pub(crate) fn context_used_percentage(&self) -> Option<f64> {
        self.context_window
            .as_ref()
            .and_then(|context_window| context_window.used_percentage)
    }

    pub(crate) fn five_hour_usage(&self) -> Option<f64> {
        self.rate_limits
            .as_ref()
            .and_then(|rate_limits| rate_limits.five_hour.as_ref())
            .and_then(|rate_limit| rate_limit.used_percentage)
    }

    pub(crate) fn five_hour_resets_at(&self) -> Option<u64> {
        self.rate_limits
            .as_ref()
            .and_then(|rate_limits| rate_limits.five_hour.as_ref())
            .and_then(|rate_limit| rate_limit.resets_at)
    }

    pub(crate) fn seven_day_usage(&self) -> Option<f64> {
        self.rate_limits
            .as_ref()
            .and_then(|rate_limits| rate_limits.seven_day.as_ref())
            .and_then(|rate_limit| rate_limit.used_percentage)
    }

    pub(crate) fn seven_day_resets_at(&self) -> Option<u64> {
        self.rate_limits
            .as_ref()
            .and_then(|rate_limits| rate_limits.seven_day.as_ref())
            .and_then(|rate_limit| rate_limit.resets_at)
    }

    fn current_usage(&self) -> Option<&CurrentUsage> {
        self.context_window
            .as_ref()
            .and_then(|context_window| context_window.current_usage.as_ref())
    }
}

pub(crate) fn parse(input: &str) -> Result<StatusInput, serde_json::Error> {
    serde_json::from_str(input)
}

fn deserialize_optional_safe_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;

    Ok(value
        .as_ref()
        .and_then(serde_json::Value::as_str)
        .map(safe_str))
}

fn deserialize_optional_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;

    Ok(value.as_ref().and_then(serde_json::Value::as_u64))
}

fn deserialize_optional_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;

    Ok(value.as_ref().and_then(serde_json::Value::as_f64))
}

fn safe_str(value: &str) -> String {
    value
        .chars()
        .filter(|character| !is_unsafe_character(*character))
        .collect()
}

const fn is_unsafe_character(character: char) -> bool {
    matches!(
        character,
        '\u{0000}'..='\u{001F}'
            | '\u{007F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2066}'..='\u{2069}'
            | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn strips_control_zero_width_and_bidi_characters_from_strings() -> Result<(), serde_json::Error>
    {
        let input = parse(
            r#"{
                "model": { "display_name": "Fa\u001bble\u200b\u202e\ufeff" }
            }"#,
        )?;

        assert_eq!(input.model_display_name(), "Fable");

        Ok(())
    }

    #[test]
    fn defaults_context_window_size_when_its_type_is_invalid() -> Result<(), serde_json::Error> {
        let input = parse(
            r#"{
                "context_window": { "context_window_size": "one million" }
            }"#,
        )?;

        assert_eq!(input.context_window_size(), 200_000);

        Ok(())
    }

    #[test]
    fn defaults_zero_context_window_size() -> Result<(), serde_json::Error> {
        let input = parse(
            r#"{
                "context_window": { "context_window_size": 0 }
            }"#,
        )?;

        assert_eq!(input.context_window_size(), 200_000);

        Ok(())
    }

    #[test]
    fn preserves_one_million_context_window_size() -> Result<(), serde_json::Error> {
        let input = parse(
            r#"{
                "context_window": { "context_window_size": 1000000 }
            }"#,
        )?;

        assert_eq!(input.context_window_size(), 1_000_000);

        Ok(())
    }

    #[test]
    fn parses_all_existing_status_input_fixtures() -> Result<(), serde_json::Error> {
        struct FixtureExpectation {
            contents: &'static str,
            session_id: &'static str,
            input_tokens: u64,
            five_hour: Option<(f64, u64)>,
            seven_day: Option<(f64, u64)>,
        }

        let fixtures = [
            FixtureExpectation {
                contents: include_str!("../tests/fixtures/status-input-boundaries.json"),
                session_id: "mock-session-boundaries",
                input_tokens: 0,
                five_hour: Some((100.0, 1_900_000_000)),
                seven_day: Some((0.0, 1_900_000_000)),
            },
            FixtureExpectation {
                contents: include_str!("../tests/fixtures/status-input-colors.json"),
                session_id: "mock-session-colors",
                input_tokens: 100_000,
                five_hour: Some((70.0, 1_900_000_000)),
                seven_day: Some((90.0, 1_900_000_000)),
            },
            FixtureExpectation {
                contents: include_str!("../tests/fixtures/status-input-oauth.json"),
                session_id: "mock-session-oauth",
                input_tokens: 50_000,
                five_hour: None,
                seven_day: None,
            },
            FixtureExpectation {
                contents: include_str!("../tests/fixtures/status-input-seven-day-only.json"),
                session_id: "mock-session-seven-day-only",
                input_tokens: 50_000,
                five_hour: None,
                seven_day: Some((50.0, 1_900_000_000)),
            },
            FixtureExpectation {
                contents: include_str!("../tests/fixtures/status-input.json"),
                session_id: "mock-session",
                input_tokens: 50_000,
                five_hour: Some((20.0, 1_900_000_000)),
                seven_day: Some((50.0, 1_900_000_000)),
            },
        ];

        for fixture in fixtures {
            let input = parse(fixture.contents)?;

            assert_eq!(input.model_display_name(), "Fable 5");
            assert_eq!(input.effort_level(), None);
            assert_eq!(input.cwd(), Some("/tmp/mock-project"));
            assert_eq!(input.session_id(), Some(fixture.session_id));
            assert_eq!(input.context_window_size(), 200_000);
            assert_eq!(input.input_tokens(), fixture.input_tokens);
            assert_eq!(input.cache_creation_input_tokens(), 0);
            assert_eq!(input.cache_read_input_tokens(), 0);
            assert_eq!(input.context_used_percentage(), None);
            assert_eq!(
                input.five_hour_usage(),
                fixture.five_hour.map(|limit| limit.0)
            );
            assert_eq!(
                input.five_hour_resets_at(),
                fixture.five_hour.map(|limit| limit.1)
            );
            assert_eq!(
                input.seven_day_usage(),
                fixture.seven_day.map(|limit| limit.0)
            );
            assert_eq!(
                input.seven_day_resets_at(),
                fixture.seven_day.map(|limit| limit.1)
            );
        }

        Ok(())
    }

    #[test]
    fn sanitizes_every_external_string_field() -> Result<(), serde_json::Error> {
        let input = parse(
            r#"{
                "model": { "display_name": "Fa\u007fble" },
                "effort": { "level": "h\u200cigh" },
                "cwd": "/tmp/\u202eproject",
                "session_id": "session\u2066id\ufeff"
            }"#,
        )?;

        assert_eq!(input.model_display_name(), "Fable");
        assert_eq!(input.effort_level(), Some("high"));
        assert_eq!(input.cwd(), Some("/tmp/project"));
        assert_eq!(input.session_id(), Some("sessionid"));

        Ok(())
    }

    #[test]
    fn defaults_every_mismatched_numeric_field() -> Result<(), serde_json::Error> {
        let input = parse(
            r#"{
                "context_window": {
                    "context_window_size": {},
                    "current_usage": {
                        "input_tokens": "50000",
                        "cache_creation_input_tokens": {},
                        "cache_read_input_tokens": []
                    },
                    "used_percentage": "50"
                },
                "rate_limits": {
                    "five_hour": { "used_percentage": {}, "resets_at": "soon" },
                    "seven_day": { "used_percentage": [], "resets_at": {} }
                }
            }"#,
        )?;

        assert_eq!(input.context_window_size(), 200_000);
        assert_eq!(input.input_tokens(), 0);
        assert_eq!(input.cache_creation_input_tokens(), 0);
        assert_eq!(input.cache_read_input_tokens(), 0);
        assert_eq!(input.context_used_percentage(), None);
        assert_eq!(input.five_hour_usage(), None);
        assert_eq!(input.five_hour_resets_at(), None);
        assert_eq!(input.seven_day_usage(), None);
        assert_eq!(input.seven_day_resets_at(), None);

        Ok(())
    }

    #[test]
    fn defaults_empty_input_object_and_ignores_unknown_fields() -> Result<(), serde_json::Error> {
        let input = parse(r#"{ "unknown": { "field": "value" } }"#)?;

        assert_eq!(input.model_display_name(), "Claude");
        assert_eq!(input.effort_level(), None);
        assert_eq!(input.cwd(), None);
        assert_eq!(input.session_id(), None);
        assert_eq!(input.context_window_size(), 200_000);
        assert_eq!(input.input_tokens(), 0);
        assert_eq!(input.cache_creation_input_tokens(), 0);
        assert_eq!(input.cache_read_input_tokens(), 0);
        assert_eq!(input.context_used_percentage(), None);
        assert_eq!(input.five_hour_usage(), None);
        assert_eq!(input.five_hour_resets_at(), None);
        assert_eq!(input.seven_day_usage(), None);
        assert_eq!(input.seven_day_resets_at(), None);

        Ok(())
    }
}
