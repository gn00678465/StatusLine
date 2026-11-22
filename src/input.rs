//! Parses and sanitizes the status-line JSON input.

use serde::{Deserialize, Deserializer};

const DEFAULT_MODEL_NAME: &str = "Claude";
const DEFAULT_AGY_MODEL_NAME: &str = "Antigravity";
const DEFAULT_CONTEXT_WINDOW_SIZE: u64 = 200_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostKind {
    ClaudeCode,
    Antigravity,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct QuotaBucket {
    pub label: String,
    pub remaining_fraction: Option<f64>,
    pub remaining_amount: Option<f64>,
    pub reset_time: Option<String>,
    pub reset_in_seconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SubagentSummary {
    pub running: usize,
    pub total: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CostSummary {
    pub total_usd: f64,
    pub estimated: bool,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct StatusInput {
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    product: Option<String>,
    #[serde(default)]
    model: Option<Model>,
    #[serde(default)]
    effort: Option<Effort>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    cwd: Option<String>,
    #[serde(default)]
    workspace: Option<Workspace>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    session_id: Option<String>,
    #[serde(default)]
    context_window: Option<ContextWindow>,
    #[serde(default)]
    rate_limits: Option<RateLimits>,
    #[serde(default, deserialize_with = "deserialize_quota_buckets")]
    quota: Vec<QuotaBucket>,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    terminal_width: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    agent_state: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    cycle_mode: Option<String>,
    #[serde(default)]
    sandbox: Option<Sandbox>,
    #[serde(default, deserialize_with = "deserialize_subagents")]
    subagents: SubagentSummary,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    artifact_count: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    task_count: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    pending_input_count: Option<u64>,
    #[serde(default, deserialize_with = "deserialize_optional_bool")]
    tool_confirmation_pending: Option<bool>,
    #[serde(default)]
    vim: Option<VimState>,
    #[serde(default)]
    cost: Option<CostInfo>,
}

#[derive(Debug, Default, Deserialize)]
struct Model {
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    display_name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    effort: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Workspace {
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    current_dir: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    project_dir: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct Sandbox {
    #[serde(default, deserialize_with = "deserialize_optional_bool")]
    enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct VimState {
    #[serde(default, deserialize_with = "deserialize_optional_safe_string")]
    mode: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct CostInfo {
    #[serde(default, deserialize_with = "deserialize_optional_f64")]
    total_usd: Option<f64>,
    #[serde(default, deserialize_with = "deserialize_optional_bool")]
    estimated: Option<bool>,
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
    pub(crate) fn host_kind(&self) -> HostKind {
        match self.product.as_deref() {
            Some(product) if product.eq_ignore_ascii_case("antigravity") => HostKind::Antigravity,
            Some(_) | None => HostKind::ClaudeCode,
        }
    }

    pub(crate) fn fallback_name(&self) -> &'static str {
        match self.host_kind() {
            HostKind::Antigravity => DEFAULT_AGY_MODEL_NAME,
            HostKind::ClaudeCode => DEFAULT_MODEL_NAME,
        }
    }

    pub(crate) fn model_display_name(&self) -> &str {
        let raw = self
            .model
            .as_ref()
            .and_then(|model| model.display_name.as_deref().or(model.id.as_deref()))
            .filter(|name| !name.is_empty());

        match (self.host_kind(), raw) {
            (HostKind::Antigravity, Some(name)) => split_model_effort_suffix(name).0,
            (HostKind::Antigravity, None) => DEFAULT_AGY_MODEL_NAME,
            (HostKind::ClaudeCode, Some(name)) => name,
            (HostKind::ClaudeCode, None) => DEFAULT_MODEL_NAME,
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
        let explicit = self
            .effort
            .as_ref()
            .and_then(|effort| effort.level.as_deref())
            .or_else(|| {
                self.model
                    .as_ref()
                    .and_then(|model| model.effort.as_deref())
            })
            .filter(|level| !level.is_empty());

        if explicit.is_some() {
            return explicit;
        }

        if self.host_kind() == HostKind::Antigravity {
            return self
                .model
                .as_ref()
                .and_then(|model| model.display_name.as_deref())
                .and_then(|name| split_model_effort_suffix(name).1);
        }

        None
    }

    pub(crate) fn cwd(&self) -> Option<&str> {
        self.cwd
            .as_deref()
            .filter(|cwd| !cwd.is_empty())
            .or_else(|| {
                self.workspace
                    .as_ref()
                    .and_then(|workspace| workspace.current_dir.as_deref())
                    .filter(|dir| !dir.is_empty())
            })
            .or_else(|| {
                self.workspace
                    .as_ref()
                    .and_then(|workspace| workspace.project_dir.as_deref())
                    .filter(|dir| !dir.is_empty())
            })
    }

    pub(crate) fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub(crate) fn terminal_width(&self) -> Option<usize> {
        self.terminal_width
            .and_then(|width| usize::try_from(width).ok())
            .filter(|&width| width > 0)
    }

    pub(crate) fn quotas(&self) -> &[QuotaBucket] {
        &self.quota
    }

    pub(crate) fn agent_state(&self) -> Option<&str> {
        self.agent_state
            .as_deref()
            .filter(|state| !state.is_empty())
    }

    pub(crate) fn cycle_mode(&self) -> Option<&str> {
        self.cycle_mode.as_deref().filter(|mode| !mode.is_empty())
    }

    pub(crate) fn sandbox_enabled(&self) -> bool {
        self.sandbox
            .as_ref()
            .and_then(|sandbox| sandbox.enabled)
            .unwrap_or(false)
    }

    pub(crate) fn subagents(&self) -> SubagentSummary {
        self.subagents
    }

    pub(crate) fn artifact_count(&self) -> u64 {
        self.artifact_count.unwrap_or(0)
    }

    pub(crate) fn task_count(&self) -> u64 {
        self.task_count.unwrap_or(0)
    }

    pub(crate) fn pending_input_count(&self) -> u64 {
        self.pending_input_count.unwrap_or(0)
    }

    pub(crate) fn tool_confirmation_pending(&self) -> bool {
        self.tool_confirmation_pending.unwrap_or(false)
    }

    pub(crate) fn vim_mode(&self) -> Option<&str> {
        self.vim
            .as_ref()
            .and_then(|vim| vim.mode.as_deref())
            .filter(|mode| !mode.is_empty())
    }

    pub(crate) fn cost(&self) -> Option<CostSummary> {
        let cost = self.cost.as_ref()?;
        let total_usd = cost
            .total_usd
            .filter(|usd| usd.is_finite() && *usd >= 0.0)?;

        Some(CostSummary {
            total_usd,
            estimated: cost.estimated.unwrap_or(false),
        })
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

fn split_model_effort_suffix(name: &str) -> (&str, Option<&'static str>) {
    let trimmed = name.trim_end();
    let Some(without_paren) = trimmed.strip_suffix(')') else {
        return (name, None);
    };
    let Some((prefix, suffix)) = without_paren.rsplit_once(" (") else {
        return (name, None);
    };
    let normalized = match suffix.trim().to_ascii_lowercase().as_str() {
        "low" => "low",
        "med" | "medium" => "medium",
        "high" => "high",
        "xhigh" => "xhigh",
        "max" => "max",
        _ => return (name, None),
    };
    let base = prefix.trim_end();
    if base.is_empty() {
        (name, None)
    } else {
        (base, Some(normalized))
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

fn deserialize_optional_bool<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;

    Ok(value.as_ref().and_then(serde_json::Value::as_bool))
}

fn deserialize_subagents<'de, D>(deserializer: D) -> Result<SubagentSummary, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(items) = value.as_ref().and_then(serde_json::Value::as_array) else {
        return Ok(SubagentSummary {
            running: 0,
            total: 0,
        });
    };

    let mut running = 0usize;
    let mut total = 0usize;
    for item in items {
        let Some(object) = item.as_object() else {
            continue;
        };
        total += 1;
        if object.get("status").and_then(serde_json::Value::as_str) == Some("running") {
            running += 1;
        }
    }

    Ok(SubagentSummary { running, total })
}

fn deserialize_quota_buckets<'de, D>(deserializer: D) -> Result<Vec<QuotaBucket>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(map) = value.as_ref().and_then(serde_json::Value::as_object) else {
        return Ok(Vec::new());
    };

    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();

    let mut buckets = Vec::new();
    for key in keys {
        let Some(object) = map.get(key).and_then(serde_json::Value::as_object) else {
            continue;
        };
        if object.get("disabled").and_then(serde_json::Value::as_bool) == Some(true) {
            continue;
        }
        let Some(label) = (match key.as_str() {
            "gemini-5h" => Some("5h".to_owned()),
            "gemini-weekly" => Some("7d".to_owned()),
            _ => None,
        }) else {
            continue;
        };
        let remaining_fraction = object
            .get("remaining_fraction")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite());
        let remaining_amount = object
            .get("remaining_amount")
            .and_then(serde_json::Value::as_f64)
            .filter(|value| value.is_finite());
        if remaining_fraction.is_none() && remaining_amount.is_none() {
            continue;
        }
        let reset_time = object
            .get("reset_time")
            .and_then(serde_json::Value::as_str)
            .map(safe_str)
            .filter(|value| !value.is_empty());
        let reset_in_seconds = object
            .get("reset_in_seconds")
            .and_then(serde_json::Value::as_u64);

        buckets.push(QuotaBucket {
            label,
            remaining_fraction,
            remaining_amount,
            reset_time,
            reset_in_seconds,
        });
    }

    Ok(buckets)
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

    #[test]
    fn parses_antigravity_payload_and_strips_model_effort_suffix() -> Result<(), serde_json::Error>
    {
        let input = parse(include_str!("../tests/fixtures/status-input-agy.json"))?;

        assert_eq!(input.host_kind(), super::HostKind::Antigravity);
        assert_eq!(input.fallback_name(), "Antigravity");
        assert_eq!(input.model_display_name(), "Gemini 4 Argon");
        assert_eq!(input.effort_level(), Some("medium"));
        assert_eq!(input.cwd(), Some("/tmp/mock-project"));
        assert_eq!(input.terminal_width(), Some(140));
        assert_eq!(input.agent_state(), Some("idle"));
        assert_eq!(input.cycle_mode(), Some("plan"));
        assert!(!input.sandbox_enabled());
        assert_eq!(input.artifact_count(), 2);
        assert_eq!(input.task_count(), 1);
        assert_eq!(input.quotas().len(), 1);
        assert_eq!(input.quotas()[0].label, "7d");

        Ok(())
    }

    #[test]
    fn preserves_non_effort_parenthetical_and_extracts_fallback_effort(
    ) -> Result<(), serde_json::Error> {
        let preview = parse(
            r#"{
                "product": "antigravity",
                "model": { "display_name": "Gemini 4 Flash (Preview)" }
            }"#,
        )?;
        assert_eq!(preview.model_display_name(), "Gemini 4 Flash (Preview)");
        assert_eq!(preview.effort_level(), None);

        let fallback_effort = parse(
            r#"{
                "product": "antigravity",
                "model": { "display_name": "Gemini 4 Hydrogen (High)" }
            }"#,
        )?;
        assert_eq!(fallback_effort.model_display_name(), "Gemini 4 Hydrogen");
        assert_eq!(fallback_effort.effort_level(), Some("high"));

        let missing_model = parse(r#"{ "product": "antigravity", "model": null }"#)?;
        assert_eq!(missing_model.model_display_name(), "Antigravity");

        Ok(())
    }

    #[test]
    fn keeps_only_gemini_5h_and_weekly_quotas_and_counts_running_subagents(
    ) -> Result<(), serde_json::Error> {
        let input = parse(
            r#"{
                "product": "antigravity",
                "quota": {
                    "disabled-bucket": { "remaining_fraction": 0.5, "disabled": true },
                    "gemini-argon-wtu-weekly": { "remaining_fraction": 0.9 },
                    "3p-5h": { "remaining_fraction": 0.4, "reset_in_seconds": 3600 },
                    "3p-weekly": { "remaining_fraction": 0.7 },
                    "gemini-5h": { "remaining_fraction": 0.25, "reset_in_seconds": 3600 },
                    "gemini-weekly": { "remaining_fraction": 0.8, "reset_in_seconds": 500000 }
                },
                "subagents": [
                    { "name": "a", "role": "research", "status": "running" },
                    { "name": "b", "role": "review", "status": "completed" }
                ]
            }"#,
        )?;

        assert_eq!(input.quotas().len(), 2);
        assert_eq!(input.quotas()[0].label, "5h");
        assert_eq!(input.quotas()[0].remaining_fraction, Some(0.25));
        assert_eq!(input.quotas()[1].label, "7d");
        assert_eq!(input.quotas()[1].remaining_fraction, Some(0.8));
        assert_eq!(input.subagents().running, 1);
        assert_eq!(input.subagents().total, 2);

        Ok(())
    }
}
