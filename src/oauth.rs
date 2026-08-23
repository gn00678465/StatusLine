//! Retrieves and caches OAuth usage information.

use std::fs;
use std::path::{Path, PathBuf};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::process::Command;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::sync::mpsc;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::thread;
use std::time::Duration;
use std::time::UNIX_EPOCH;

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::cachedir::{CacheDir, Clock, SystemClock};

const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
const CACHE_TTL_SECONDS: u64 = 60;
const LOCK_MAX_AGE_SECONDS: u64 = 30;
const OAUTH_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";

pub trait CredentialStore {
    fn environment_token(&self) -> Option<String>;
    fn macos_keychain_blob(&self, service: &str) -> Option<String>;
    fn credentials_file_blob(&self, config_dir: &Path) -> Option<String>;
    fn linux_secret_tool_blob(&self) -> Option<String>;
}

pub trait HttpClient {
    fn get_usage(&self, token: &str) -> Option<String>;
}

pub struct SystemCredentialStore;

impl CredentialStore for SystemCredentialStore {
    fn environment_token(&self) -> Option<String> {
        std::env::var("CLAUDE_CODE_OAUTH_TOKEN").ok()
    }

    fn macos_keychain_blob(&self, service: &str) -> Option<String> {
        #[cfg(target_os = "macos")]
        {
            run_command_with_timeout(
                "security",
                vec![
                    "find-generic-password".to_owned(),
                    "-s".to_owned(),
                    service.to_owned(),
                    "-w".to_owned(),
                ],
                Duration::from_secs(3),
            )
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _unused_service = service;

            None
        }
    }

    fn credentials_file_blob(&self, config_dir: &Path) -> Option<String> {
        fs::read_to_string(config_dir.join(".credentials.json")).ok()
    }

    fn linux_secret_tool_blob(&self) -> Option<String> {
        #[cfg(target_os = "linux")]
        {
            run_command_with_timeout(
                "secret-tool",
                vec![
                    "lookup".to_owned(),
                    "service".to_owned(),
                    KEYCHAIN_SERVICE.to_owned(),
                ],
                Duration::from_secs(2),
            )
        }

        #[cfg(not(target_os = "linux"))]
        {
            None
        }
    }
}

pub struct UreqHttpClient;

impl HttpClient for UreqHttpClient {
    fn get_usage(&self, token: &str) -> Option<String> {
        let agent = ureq::Agent::config_builder()
            .timeout_connect(Some(Duration::from_secs(3)))
            .timeout_global(Some(Duration::from_secs(10)))
            .build()
            .new_agent();
        let authorization = format!("Bearer {token}");
        let mut response = agent
            .get(OAUTH_USAGE_URL)
            .header("Authorization", &authorization)
            .header("Accept", "application/json")
            .header("Content-Type", "application/json")
            .header("anthropic-beta", "oauth-2025-04-20")
            .header("User-Agent", "claude-code/2.1.34")
            .call()
            .ok()?;

        response.body_mut().read_to_string().ok()
    }
}

pub fn system_usage_fetcher(
    cache_dir: CacheDir,
) -> OAuthUsageFetcher<SystemCredentialStore, UreqHttpClient, SystemClock> {
    OAuthUsageFetcher::new(
        SystemCredentialStore,
        UreqHttpClient,
        cache_dir,
        configured_dir_from_environment(),
        SystemClock,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct UsageResponse {
    pub five_hour: UsageWindow,
    pub seven_day: UsageWindow,
    pub extra_usage: ExtraUsage,
    pub weekly_scoped: Vec<WeeklyScopedUsage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageWindow {
    pub utilization: u8,
    pub resets_at: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExtraUsage {
    pub is_enabled: bool,
    pub utilization: u8,
    pub used_credits: f64,
    pub monthly_limit: f64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeeklyScopedUsage {
    pub display_name: String,
    pub percent: u8,
    pub resets_at: String,
}

pub struct OAuthUsageFetcher<S, H, C> {
    credentials: S,
    http: H,
    cache_dir: CacheDir,
    configured_dir: Option<PathBuf>,
    clock: C,
}

impl<S, H, C> OAuthUsageFetcher<S, H, C>
where
    S: CredentialStore,
    H: HttpClient,
    C: Clock,
{
    pub fn new(
        credentials: S,
        http: H,
        cache_dir: CacheDir,
        configured_dir: Option<PathBuf>,
        clock: C,
    ) -> Self {
        Self {
            credentials,
            http,
            cache_dir,
            configured_dir,
            clock,
        }
    }

    pub fn fetch(&self) -> Option<UsageResponse> {
        let config_dir = self.config_dir();
        let cache_name = cache_entry_name(&config_dir);
        let cached_usage = self.read_cached_usage(&cache_name);

        if cached_usage.is_some() && self.cache_is_fresh(&cache_name) {
            return cached_usage;
        }

        if !self.cache_dir.is_safe() {
            return self
                .fetch_remote(&config_dir)
                .map(|(_, usage)| usage)
                .or(cached_usage);
        }

        let lock = self
            .cache_dir
            .try_lock(&cache_name, LOCK_MAX_AGE_SECONDS, &self.clock);
        if lock.is_none() {
            return cached_usage;
        }

        let refreshed_usage = self.fetch_remote(&config_dir);
        if let Some((response, _)) = refreshed_usage.as_ref() {
            let _write_succeeded = self
                .cache_dir
                .atomic_write(&cache_name, response.as_bytes());
        }
        drop(lock);

        refreshed_usage.map(|(_, usage)| usage).or(cached_usage)
    }

    fn fetch_remote(&self, config_dir: &Path) -> Option<(String, UsageResponse)> {
        let token = self.token(config_dir)?;
        let response = self.http.get_usage(&token)?;
        let usage = parse_usage(&response)?;

        Some((response, usage))
    }

    fn read_cached_usage(&self, cache_name: &str) -> Option<UsageResponse> {
        let contents = self.cache_dir.read(cache_name)?;
        let contents = std::str::from_utf8(&contents).ok()?;

        parse_usage(contents)
    }

    fn cache_is_fresh(&self, cache_name: &str) -> bool {
        let cache_path = match self.cache_dir.path() {
            Some(cache_dir) => cache_dir.join(cache_name),
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

    fn config_dir(&self) -> PathBuf {
        match self.configured_dir.as_ref() {
            Some(config_dir) => config_dir.clone(),
            None => default_config_dir(),
        }
    }

    fn token(&self, config_dir: &Path) -> Option<String> {
        valid_token(self.credentials.environment_token())
            .or_else(|| {
                credential_token(
                    self.credentials
                        .macos_keychain_blob(&keychain_service(self.configured_dir.as_deref())),
                )
            })
            .or_else(|| credential_token(self.credentials.credentials_file_blob(config_dir)))
            .or_else(|| credential_token(self.credentials.linux_secret_tool_blob()))
    }
}

pub fn parse_usage(response: &str) -> Option<UsageResponse> {
    let response: Value = serde_json::from_str(response).ok()?;
    let five_hour = parse_window(response.get("five_hour"))?;
    let seven_day = match parse_window(response.get("seven_day")) {
        Some(window) => window,
        None => UsageWindow {
            utilization: 0,
            resets_at: String::new(),
        },
    };

    let weekly_scoped = match response.get("limits").and_then(Value::as_array) {
        Some(limits) => limits.iter().filter_map(parse_weekly_scoped).collect(),
        None => Vec::new(),
    };

    Some(UsageResponse {
        five_hour,
        seven_day,
        extra_usage: parse_extra_usage(response.get("extra_usage")),
        weekly_scoped,
    })
}

fn parse_window(value: Option<&Value>) -> Option<UsageWindow> {
    let value = value?.as_object()?;

    Some(UsageWindow {
        utilization: rounded_percentage(value.get("utilization")),
        resets_at: external_string(value.get("resets_at")),
    })
}

fn parse_extra_usage(value: Option<&Value>) -> ExtraUsage {
    let value = match value.and_then(Value::as_object) {
        Some(value) => value,
        None => {
            return ExtraUsage {
                is_enabled: false,
                utilization: 0,
                used_credits: 0.0,
                monthly_limit: 0.0,
            };
        }
    };

    ExtraUsage {
        is_enabled: matches!(value.get("is_enabled").and_then(Value::as_bool), Some(true)),
        utilization: rounded_percentage(value.get("utilization")),
        used_credits: numeric_value(value.get("used_credits")),
        monthly_limit: numeric_value(value.get("monthly_limit")),
    }
}

fn parse_weekly_scoped(value: &Value) -> Option<WeeklyScopedUsage> {
    let value = value.as_object()?;
    if value.get("kind").and_then(Value::as_str) != Some("weekly_scoped") {
        return None;
    }

    let display_name = value
        .get("scope")
        .and_then(Value::as_object)
        .and_then(|scope| scope.get("model"))
        .and_then(Value::as_object)
        .and_then(|model| model.get("display_name"));

    Some(WeeklyScopedUsage {
        display_name: external_string(display_name),
        percent: rounded_percentage(value.get("percent")),
        resets_at: external_string(value.get("resets_at")),
    })
}

fn rounded_percentage(value: Option<&Value>) -> u8 {
    let percentage = numeric_value(value);
    if percentage <= 0.0 {
        0
    } else if percentage >= 100.0 {
        100
    } else {
        percentage.round() as u8
    }
}

fn numeric_value(value: Option<&Value>) -> f64 {
    let number = match value {
        Some(Value::Number(number)) => number.as_f64(),
        Some(Value::String(number)) => number.parse::<f64>().ok(),
        _ => None,
    };

    match number {
        Some(number) if number.is_finite() => number,
        Some(_) | None => 0.0,
    }
}

fn external_string(value: Option<&Value>) -> String {
    match value.and_then(Value::as_str) {
        Some(value) => value
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
            .collect(),
        None => String::new(),
    }
}

fn cache_entry_name(config_dir: &Path) -> String {
    let config_dir_key: String = config_dir
        .to_string_lossy()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .take(64)
        .collect();

    format!("usage-cache-{config_dir_key}.json")
}

fn valid_token(token: Option<String>) -> Option<String> {
    token.filter(|token| !token.is_empty() && token != "null")
}

fn credential_token(blob: Option<String>) -> Option<String> {
    #[derive(Deserialize)]
    struct CredentialsBlob {
        #[serde(rename = "claudeAiOauth")]
        oauth: Option<OAuthCredentials>,
    }

    #[derive(Deserialize)]
    struct OAuthCredentials {
        #[serde(rename = "accessToken")]
        access_token: Option<String>,
    }

    let blob = blob?;
    let credentials: CredentialsBlob = serde_json::from_str(&blob).ok()?;
    valid_token(credentials.oauth.and_then(|oauth| oauth.access_token))
}

fn keychain_service(config_dir: Option<&Path>) -> String {
    match config_dir {
        Some(config_dir) => {
            let digest = Sha256::digest(config_dir.to_string_lossy().as_bytes());
            let mut prefix = String::with_capacity(8);
            for byte in digest.iter().take(4) {
                use std::fmt::Write;

                let _write_result = write!(&mut prefix, "{byte:02x}");
            }
            format!("{KEYCHAIN_SERVICE}-{prefix}")
        }
        None => KEYCHAIN_SERVICE.to_owned(),
    }
}

fn default_config_dir() -> PathBuf {
    home_directory().map_or_else(|| PathBuf::from(".claude"), |home| home.join(".claude"))
}

fn configured_dir_from_environment() -> Option<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(config_dir) if !config_dir.is_empty() => Some(PathBuf::from(config_dir)),
        Some(_) | None => None,
    }
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

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn run_command_with_timeout(
    command: &str,
    arguments: Vec<String>,
    timeout: Duration,
) -> Option<String> {
    let command = command.to_owned();
    let (sender, receiver) = mpsc::sync_channel(1);
    let _thread = thread::spawn(move || {
        let output = Command::new(command).args(arguments).output();
        let stdout = match output {
            Ok(output) if output.status.success() => String::from_utf8(output.stdout).ok(),
            Ok(_) | Err(_) => None,
        };
        let _send_result = sender.send(stdout);
    });

    receiver.recv_timeout(timeout).ok().flatten()
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use tempfile::tempdir;

    use crate::cachedir::CacheDir;

    use super::{
        cache_entry_name, keychain_service, parse_usage, CredentialStore, HttpClient,
        OAuthUsageFetcher, UsageResponse,
    };

    const VALID_USAGE: &str = r#"{
        "five_hour": {
            "utilization": 20,
            "resets_at": "2030-03-17T17:46:40Z"
        }
    }"#;

    #[derive(Clone)]
    struct MockCredentials {
        environment: Option<String>,
        keychain: Option<String>,
        file: Option<String>,
        secret_tool: Option<String>,
    }

    impl CredentialStore for MockCredentials {
        fn environment_token(&self) -> Option<String> {
            self.environment.clone()
        }

        fn macos_keychain_blob(&self, _service: &str) -> Option<String> {
            self.keychain.clone()
        }

        fn credentials_file_blob(&self, _config_dir: &std::path::Path) -> Option<String> {
            self.file.clone()
        }

        fn linux_secret_tool_blob(&self) -> Option<String> {
            self.secret_tool.clone()
        }
    }

    #[derive(Clone)]
    struct MockHttp {
        expected_token: String,
        response: String,
        calls: Arc<AtomicUsize>,
    }

    impl HttpClient for MockHttp {
        fn get_usage(&self, token: &str) -> Option<String> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            (token == self.expected_token).then(|| self.response.clone())
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
    fn fetch_prefers_the_environment_token_over_every_other_source() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let fetcher = OAuthUsageFetcher::new(
            MockCredentials {
                environment: Some("environment-token".to_owned()),
                keychain: Some(r#"{"claudeAiOauth":{"accessToken":"keychain-token"}}"#.to_owned()),
                file: Some(r#"{"claudeAiOauth":{"accessToken":"file-token"}}"#.to_owned()),
                secret_tool: Some(r#"{"claudeAiOauth":{"accessToken":"secret-token"}}"#.to_owned()),
            },
            MockHttp {
                expected_token: "environment-token".to_owned(),
                response: VALID_USAGE.to_owned(),
                calls: Arc::clone(&calls),
            },
            CacheDir::from_paths(None, Some(home.path())),
            Some(PathBuf::from("/custom/claude")),
            FixedClock { now: 100 },
        );

        let usage: UsageResponse = fetcher.fetch().ok_or("missing OAuth usage")?;

        assert_eq!(usage.five_hour.utilization, 20);
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        Ok(())
    }

    #[test]
    fn fetches_from_each_credential_source_in_priority_order() -> Result<(), Box<dyn Error>> {
        let sources = [
            (
                "keychain-token",
                MockCredentials {
                    environment: None,
                    keychain: Some(
                        r#"{"claudeAiOauth":{"accessToken":"keychain-token"}}"#.to_owned(),
                    ),
                    file: Some(r#"{"claudeAiOauth":{"accessToken":"file-token"}}"#.to_owned()),
                    secret_tool: Some(
                        r#"{"claudeAiOauth":{"accessToken":"secret-token"}}"#.to_owned(),
                    ),
                },
            ),
            (
                "file-token",
                MockCredentials {
                    environment: None,
                    keychain: None,
                    file: Some(r#"{"claudeAiOauth":{"accessToken":"file-token"}}"#.to_owned()),
                    secret_tool: Some(
                        r#"{"claudeAiOauth":{"accessToken":"secret-token"}}"#.to_owned(),
                    ),
                },
            ),
            (
                "secret-token",
                MockCredentials {
                    environment: None,
                    keychain: None,
                    file: None,
                    secret_tool: Some(
                        r#"{"claudeAiOauth":{"accessToken":"secret-token"}}"#.to_owned(),
                    ),
                },
            ),
        ];

        for (expected_token, credentials) in sources {
            let home = tempdir()?;
            let calls = Arc::new(AtomicUsize::new(0));
            let fetcher = OAuthUsageFetcher::new(
                credentials,
                MockHttp {
                    expected_token: expected_token.to_owned(),
                    response: VALID_USAGE.to_owned(),
                    calls: Arc::clone(&calls),
                },
                CacheDir::from_paths(None, Some(home.path())),
                Some(PathBuf::from("/custom/claude")),
                FixedClock { now: 100 },
            );

            assert!(fetcher.fetch().is_some(), "{expected_token}");
            assert_eq!(calls.load(Ordering::Relaxed), 1, "{expected_token}");
        }

        Ok(())
    }

    #[test]
    fn skips_the_http_request_when_all_token_sources_are_empty() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let fetcher = OAuthUsageFetcher::new(
            MockCredentials {
                environment: None,
                keychain: None,
                file: None,
                secret_tool: None,
            },
            MockHttp {
                expected_token: String::new(),
                response: VALID_USAGE.to_owned(),
                calls: Arc::clone(&calls),
            },
            CacheDir::from_paths(None, Some(home.path())),
            Some(PathBuf::from("/custom/claude")),
            FixedClock { now: 100 },
        );

        assert!(fetcher.fetch().is_none());
        assert_eq!(calls.load(Ordering::Relaxed), 0);

        Ok(())
    }

    #[test]
    fn returns_a_fresh_usage_cache_without_a_second_request() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let calls = Arc::new(AtomicUsize::new(0));
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let fetcher = OAuthUsageFetcher::new(
            MockCredentials {
                environment: Some("environment-token".to_owned()),
                keychain: None,
                file: None,
                secret_tool: None,
            },
            MockHttp {
                expected_token: "environment-token".to_owned(),
                response: VALID_USAGE.to_owned(),
                calls: Arc::clone(&calls),
            },
            CacheDir::from_paths(None, Some(home.path())),
            Some(PathBuf::from("/custom/claude")),
            FixedClock { now },
        );

        assert!(fetcher.fetch().is_some());
        assert!(fetcher.fetch().is_some());
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        Ok(())
    }

    #[test]
    fn returns_stale_cache_when_another_process_holds_the_refresh_lock(
    ) -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let config_dir = Path::new("/custom/claude");
        let cache_name = cache_entry_name(config_dir);
        let cache_dir = CacheDir::from_paths(None, Some(home.path()));
        assert!(cache_dir.atomic_write(&cache_name, VALID_USAGE.as_bytes()));
        let held_lock = cache_dir
            .try_lock(&cache_name, 30, &FixedClock { now: 0 })
            .ok_or("failed to hold the usage cache lock")?;

        let calls = Arc::new(AtomicUsize::new(0));
        let fetcher = OAuthUsageFetcher::new(
            MockCredentials {
                environment: Some("environment-token".to_owned()),
                keychain: None,
                file: None,
                secret_tool: None,
            },
            MockHttp {
                expected_token: "environment-token".to_owned(),
                response: VALID_USAGE.to_owned(),
                calls: Arc::clone(&calls),
            },
            CacheDir::from_paths(None, Some(home.path())),
            Some(config_dir.to_path_buf()),
            FixedClock { now: 0 },
        );

        let usage = fetcher.fetch().ok_or("missing stale usage")?;
        assert_eq!(usage.five_hour.utilization, 20);
        assert_eq!(calls.load(Ordering::Relaxed), 0);

        drop(held_lock);

        Ok(())
    }

    #[test]
    fn does_not_cache_a_response_without_five_hour() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let config_dir = Path::new("/custom/claude");
        let cache_name = cache_entry_name(config_dir);
        let fetcher = OAuthUsageFetcher::new(
            MockCredentials {
                environment: Some("environment-token".to_owned()),
                keychain: None,
                file: None,
                secret_tool: None,
            },
            MockHttp {
                expected_token: "environment-token".to_owned(),
                response: r#"{"seven_day":{"utilization":50}}"#.to_owned(),
                calls: Arc::new(AtomicUsize::new(0)),
            },
            CacheDir::from_paths(None, Some(home.path())),
            Some(config_dir.to_path_buf()),
            FixedClock { now: 100 },
        );

        assert!(fetcher.fetch().is_none());
        assert!(CacheDir::from_paths(None, Some(home.path()))
            .read(&cache_name)
            .is_none());

        Ok(())
    }

    #[test]
    fn parses_every_weekly_scoped_limit_and_extra_usage_variants() -> Result<(), Box<dyn Error>> {
        let fixture_input =
            crate::input::parse(include_str!("../tests/fixtures/status-input-oauth.json"))?;
        assert_eq!(fixture_input.five_hour_usage(), None);
        assert_eq!(fixture_input.seven_day_usage(), None);

        let no_weekly = parse_usage(
            r#"{
                "five_hour":{"utilization":20,"resets_at":"2030-03-17T17:46:40Z"},
                "seven_day":{"utilization":50,"resets_at":"2030-03-24T17:46:40Z"},
                "extra_usage":{"is_enabled":false},
                "limits":[]
            }"#,
        )
        .ok_or("failed to parse zero weekly limits")?;
        assert!(no_weekly.weekly_scoped.is_empty());
        assert!(!no_weekly.extra_usage.is_enabled);

        let one_weekly = parse_usage(
            r#"{
                "five_hour":{"utilization":"20.4","resets_at":"2030-03-17T17:46:40Z"},
                "seven_day":{"utilization":50,"resets_at":"2030-03-24T17:46:40Z"},
                "extra_usage":{"is_enabled":true,"utilization":12.5,"used_credits":1250,"monthly_limit":10000},
                "limits":[{"kind":"weekly_scoped","scope":{"model":{"display_name":"Fable 5"}},"percent":30.5,"resets_at":"2030-03-24T17:46:40Z"}]
            }"#,
        )
        .ok_or("failed to parse one weekly limit")?;
        assert_eq!(one_weekly.five_hour.utilization, 20);
        assert_eq!(one_weekly.seven_day.utilization, 50);
        assert!(one_weekly.extra_usage.is_enabled);
        assert_eq!(one_weekly.extra_usage.utilization, 13);
        assert_eq!(one_weekly.extra_usage.used_credits, 1_250.0);
        assert_eq!(one_weekly.extra_usage.monthly_limit, 10_000.0);
        assert_eq!(one_weekly.weekly_scoped.len(), 1);
        assert_eq!(one_weekly.weekly_scoped[0].display_name, "Fable 5");
        assert_eq!(one_weekly.weekly_scoped[0].percent, 31);

        let multiple_weekly = parse_usage(
            r#"{
                "five_hour":{"utilization":0,"resets_at":"2030-03-17T17:46:40Z"},
                "limits":[
                    {"kind":"daily","scope":{"model":{"display_name":"Ignore"}},"percent":99},
                    {"kind":"weekly_scoped","scope":{"model":{"display_name":"Haiku"}},"percent":20,"resets_at":"2030-03-24T17:46:40Z"},
                    {"kind":"weekly_scoped","scope":{"model":{"display_name":"Sonnet"}},"percent":40,"resets_at":"2030-03-31T17:46:40Z"}
                ]
            }"#,
        )
        .ok_or("failed to parse multiple weekly limits")?;
        assert_eq!(multiple_weekly.weekly_scoped.len(), 2);
        assert_eq!(multiple_weekly.weekly_scoped[0].display_name, "Haiku");
        assert_eq!(multiple_weekly.weekly_scoped[1].display_name, "Sonnet");

        Ok(())
    }

    #[test]
    fn uses_the_sha256_scoped_keychain_service_for_a_custom_config_dir() {
        assert_eq!(
            keychain_service(Some(Path::new("/custom/claude"))),
            "Claude Code-credentials-7427f042"
        );
        assert_eq!(keychain_service(None), "Claude Code-credentials");
    }

    #[test]
    fn sanitizes_and_limits_the_usage_cache_key_to_shell_semantics() {
        assert_eq!(
            cache_entry_name(Path::new("/Users/name/.claude with spaces")),
            "usage-cache-_Users_name__claude_with_spaces.json"
        );

        let long_directory = format!("/{}", "a".repeat(80));
        assert_eq!(
            cache_entry_name(Path::new(&long_directory)),
            format!("usage-cache-_{}.json", "a".repeat(63))
        );
    }
}
