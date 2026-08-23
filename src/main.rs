#![deny(warnings, clippy::expect_used, clippy::unwrap_used)]

//! Entry point for the `cc-statusline` command-line application.

pub mod cachedir;
mod config;
pub mod gitstatus;
mod input;
pub mod oauth;
pub mod render;
pub mod ttl;
pub mod update;
pub mod width;

use std::io::Read;
use std::path::{Path, PathBuf};

use cachedir::{CacheDir, Clock, SystemClock};
use config::{Config, UsageStyle};
use gitstatus::{CommandGitRunner, GitRunner, GitStatus, GitStatusCollector};
use oauth::{
    CredentialStore, HttpClient, OAuthUsageFetcher, SystemCredentialStore, UreqHttpClient,
};
use render::blocks::ContextUsage;
use render::limits::{
    render_limits, BuiltInLimit, BuiltInLimits, LimitRenderContext, LocalOffset, SystemLocalOffset,
};
use render::meter::MeterStyle;
use render::{render, RenderContext};
use ttl::{CacheTtl, TokenUsage};
use update::UpdateChecker;

const FALLBACK_OUTPUT: &str = "Claude";

fn main() {
    let input = read_stdin();
    let output = match std::panic::catch_unwind(|| render_system_input(&input)) {
        Ok(output) if !output.is_empty() => output,
        Ok(_) | Err(_) => FALLBACK_OUTPUT.to_owned(),
    };

    print!("{output}");
}

fn read_stdin() -> String {
    let mut input = String::new();
    let _read_result = std::io::stdin().read_to_string(&mut input);

    input
}

fn render_system_input(input: &str) -> String {
    let parsed_input = match input::parse(input) {
        Ok(parsed_input) => parsed_input,
        Err(_) => return FALLBACK_OUTPUT.to_owned(),
    };
    let app = StatusLineApp::system();

    app.render_status(&parsed_input)
}

struct StatusLineApp<R, S, H, C, O> {
    git_runner: R,
    credentials: S,
    http: H,
    cache_dir: CacheDir,
    clock: C,
    offset: O,
    config: Config,
    configured_dir: Option<PathBuf>,
}

impl<R, S, H, C, O> StatusLineApp<R, S, H, C, O>
where
    R: GitRunner + Clone,
    S: CredentialStore + Clone,
    H: HttpClient + Clone,
    C: Clock + Clone,
    O: LocalOffset,
{
    fn new(
        git_runner: R,
        credentials: S,
        http: H,
        cache_dir: CacheDir,
        clock: C,
        offset: O,
        config: Config,
    ) -> Self {
        Self {
            git_runner,
            credentials,
            http,
            cache_dir,
            clock,
            offset,
            config,
            configured_dir: None,
        }
    }

    #[cfg(test)]
    fn render_input(&self, input: &str) -> String {
        match input::parse(input) {
            Ok(parsed_input) => self.render_status(&parsed_input),
            Err(_) => FALLBACK_OUTPUT.to_owned(),
        }
    }

    fn render_status(&self, parsed_input: &input::StatusInput) -> String {
        let session_id = session_key(parsed_input);
        let git_status = self.collect_git_status(parsed_input, &session_id);
        let cache_status = CacheTtl::new(self.cache_dir.clone(), &session_id, self.clock.clone())
            .update(TokenUsage {
                input_tokens: parsed_input.input_tokens(),
                cache_creation_tokens: parsed_input.cache_creation_input_tokens(),
                cache_read_tokens: parsed_input.cache_read_input_tokens(),
            });
        let oauth_usage = OAuthUsageFetcher::new(
            self.credentials.clone(),
            self.http.clone(),
            self.cache_dir.clone(),
            self.configured_dir.clone(),
            self.clock.clone(),
        )
        .fetch();
        let limits = render_limits(
            LimitRenderContext {
                builtin: built_in_limits(parsed_input),
                oauth: oauth_usage.as_ref(),
                meter_style: meter_style(self.config.usage_style()),
            },
            &self.offset,
        );
        let status_line = render(RenderContext {
            cwd: parsed_input.cwd(),
            git_status: &git_status,
            model_name: parsed_input.model_display_name(),
            effort_level: parsed_input.effort_level(),
            context_usage: ContextUsage {
                input_tokens: parsed_input.input_tokens(),
                cache_creation_tokens: parsed_input.cache_creation_input_tokens(),
                cache_read_tokens: parsed_input.cache_read_input_tokens(),
                context_window_size: parsed_input.context_window_size(),
                official_percentage: parsed_input.context_used_percentage(),
            },
            cache_status,
            limits: &limits,
            meter_style: meter_style(self.config.usage_style()),
            columns: self.config.columns(),
        });
        let update_line = UpdateChecker::new(
            self.http.clone(),
            self.cache_dir.clone(),
            self.clock.clone(),
        )
        .check();

        format!("{status_line}{update_line}")
    }

    fn collect_git_status(&self, parsed_input: &input::StatusInput, session_id: &str) -> GitStatus {
        match parsed_input.cwd() {
            Some(cwd) => GitStatusCollector::new(
                self.git_runner.clone(),
                self.cache_dir.clone(),
                self.clock.clone(),
            )
            .collect(
                Path::new(cwd),
                session_id,
                self.config.git_cache_ttl_seconds(),
            ),
            None => GitStatus::default(),
        }
    }
}

impl
    StatusLineApp<
        CommandGitRunner,
        SystemCredentialStore,
        UreqHttpClient,
        SystemClock,
        SystemLocalOffset,
    >
{
    fn system() -> Self {
        let mut app = Self::new(
            CommandGitRunner,
            SystemCredentialStore,
            UreqHttpClient,
            CacheDir::from_environment(),
            SystemClock,
            SystemLocalOffset,
            Config::from_env(),
        );
        app.configured_dir = configured_dir_from_environment();

        app
    }
}

fn session_key(input: &input::StatusInput) -> String {
    input
        .session_id()
        .filter(|session_id| !session_id.is_empty())
        .or_else(|| input.cwd().filter(|cwd| !cwd.is_empty()))
        .unwrap_or("no-cwd")
        .to_owned()
}

fn built_in_limits(input: &input::StatusInput) -> BuiltInLimits {
    BuiltInLimits {
        five_hour: input.five_hour_usage().map(|used_percentage| BuiltInLimit {
            used_percentage,
            resets_at: input.five_hour_resets_at(),
        }),
        seven_day: input.seven_day_usage().map(|used_percentage| BuiltInLimit {
            used_percentage,
            resets_at: input.seven_day_resets_at(),
        }),
    }
}

fn meter_style(usage_style: UsageStyle) -> MeterStyle {
    match usage_style {
        UsageStyle::Bar => MeterStyle::Bar,
        UsageStyle::Dots => MeterStyle::Dots,
    }
}

fn configured_dir_from_environment() -> Option<PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(config_dir) if !config_dir.is_empty() => Some(PathBuf::from(config_dir)),
        Some(_) | None => None,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::path::Path;

    use insta::assert_snapshot;
    use tempfile::tempdir;

    use crate::cachedir::CacheDir;
    use crate::config::Config;
    use crate::gitstatus::{GitRunResult, GitRunner};
    use crate::oauth::{CredentialStore, HttpClient};
    use crate::render::limits::LocalOffset;
    use crate::width::strip_ansi;

    use super::{session_key, StatusLineApp};

    #[derive(Clone)]
    struct MockGit;

    impl GitRunner for MockGit {
        fn run_status(&self, _repository: &Path) -> GitRunResult {
            GitRunResult::Completed {
                stdout: "# branch.head integration\n1 M. N... 100644 100644 100644 abc def staged.rs\n2 .M N... 100644 100644 100644 abc def R100 old.rs\tnew.rs\n".to_owned(),
                success: true,
            }
        }
    }

    #[derive(Clone)]
    struct MockCredentials;

    impl CredentialStore for MockCredentials {
        fn environment_token(&self) -> Option<String> {
            Some("mock-token".to_owned())
        }

        fn macos_keychain_blob(&self, _service: &str) -> Option<String> {
            None
        }

        fn credentials_file_blob(&self, _config_dir: &Path) -> Option<String> {
            None
        }

        fn linux_secret_tool_blob(&self) -> Option<String> {
            None
        }
    }

    #[derive(Clone)]
    struct MockHttp;

    impl HttpClient for MockHttp {
        fn get_usage(&self, _token: &str) -> Option<String> {
            Some(
                r#"{
                    "five_hour":{"utilization":20,"resets_at":"2030-03-17T17:46:40Z"},
                    "seven_day":{"utilization":50,"resets_at":"2030-03-24T17:46:40Z"},
                    "extra_usage":{"is_enabled":false},
                    "limits":[
                        {"kind":"weekly_scoped","scope":{"model":{"display_name":"Other"}},"percent":99,"resets_at":"2030-03-24T17:46:40Z"},
                        {"kind":"weekly_scoped","scope":{"model":{"display_name":"Fable"}},"percent":30,"resets_at":"2030-03-24T17:46:40Z"}
                    ]
                }"#
                .to_owned(),
            )
        }

        fn get_url(&self, _url: &str) -> Option<String> {
            Some(r#"{"tag_name":"v0.1.0"}"#.to_owned())
        }
    }

    #[derive(Clone)]
    struct FixedClock;

    impl crate::cachedir::Clock for FixedClock {
        fn now_epoch(&self) -> u64 {
            1_900_000_000
        }
    }

    struct UtcOffset;

    impl LocalOffset for UtcOffset {
        fn offset_seconds(&self, _epoch_seconds: i64) -> i32 {
            0
        }
    }

    fn test_app(
        home: &Path,
        columns: &str,
    ) -> StatusLineApp<MockGit, MockCredentials, MockHttp, FixedClock, UtcOffset> {
        StatusLineApp::new(
            MockGit,
            MockCredentials,
            MockHttp,
            CacheDir::from_paths(None, Some(home)),
            FixedClock,
            UtcOffset,
            Config::from_values(Some("bar"), Some("2"), Some(columns)),
        )
    }

    #[test]
    fn returns_claude_for_empty_or_invalid_stdin() -> Result<(), Box<dyn Error>> {
        let home = tempdir()?;
        let app = test_app(home.path(), "100");

        assert_eq!(app.render_input(""), "Claude");
        assert_eq!(app.render_input("not-json"), "Claude");

        Ok(())
    }

    #[test]
    fn snapshots_every_status_input_fixture_with_injected_adapters() -> Result<(), Box<dyn Error>> {
        let fixtures = [
            (
                "default",
                include_str!("../tests/fixtures/status-input.json"),
                "1000",
            ),
            (
                "boundaries",
                include_str!("../tests/fixtures/status-input-boundaries.json"),
                "1000",
            ),
            (
                "colors",
                include_str!("../tests/fixtures/status-input-colors.json"),
                "1000",
            ),
            (
                "oauth",
                include_str!("../tests/fixtures/status-input-oauth.json"),
                "1000",
            ),
            (
                "seven-day-only",
                include_str!("../tests/fixtures/status-input-seven-day-only.json"),
                "1000",
            ),
            (
                "xhigh",
                include_str!("../tests/fixtures/status-input-xhigh.json"),
                "1000",
            ),
            (
                "effort-missing",
                include_str!("../tests/fixtures/status-input-effort-missing.json"),
                "1000",
            ),
            (
                "one-million",
                include_str!("../tests/fixtures/status-input-one-million.json"),
                "1000",
            ),
            (
                "multiple-weekly",
                include_str!("../tests/fixtures/status-input-multiple-weekly.json"),
                "1000",
            ),
            (
                "wrapped",
                include_str!("../tests/fixtures/status-input-wrapped.json"),
                "80",
            ),
        ];
        let mut rendered = Vec::new();

        for (name, fixture, columns) in fixtures {
            let home = tempdir()?;
            let app = test_app(home.path(), columns);
            rendered.push(format!(
                "--- {name}\n{}",
                strip_ansi(&app.render_input(fixture))
            ));
        }

        assert_snapshot!(rendered.join("\n"), @r###"
--- default
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Fable 5 │ ⚡️ 50k/200k (▓▓░░░░░░░░ 25%) · Cache 0% 60:00 · 📊 5h: ▓▓░░░░░░░░ 20% @17:46 · 7d: ▓▓▓▓▓░░░░░ 50% @Mar 17, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
--- boundaries
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Fable 5 │ ⚡️ 0/200k (░░░░░░░░░░ 0%) · 📊 5h: ▓▓▓▓▓▓▓▓▓▓ 100% @17:46 · 7d: ░░░░░░░░░░ 0% @Mar 17, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
--- colors
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Fable 5 │ ⚡️ 100k/200k (▓▓▓▓▓░░░░░ 50%) · Cache 0% 60:00 · 📊 5h: ▓▓▓▓▓▓▓░░░ 70% @17:46 · 7d: ▓▓▓▓▓▓▓▓▓░ 90% @Mar 17, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
--- oauth
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Fable 5 │ ⚡️ 50k/200k (▓▓░░░░░░░░ 25%) · Cache 0% 60:00 · 📊 5h: ▓▓░░░░░░░░ 20% @17:46 · 7d: ▓▓▓▓▓░░░░░ 50% @Mar 24, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
--- seven-day-only
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Fable 5 │ ⚡️ 50k/200k (▓▓░░░░░░░░ 25%) · Cache 0% 60:00 · 📊 7d: ▓▓▓▓▓░░░░░ 50% @Mar 17, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
--- xhigh
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Sonnet 4.5 · 🧠 xhigh │ ⚡️ 50k/200k (▓▓░░░░░░░░ 25%) · Cache 0% 60:00 · 📊 5h: ▓▓░░░░░░░░ 20% @17:46 · 7d: ▓▓▓▓▓░░░░░ 50% @Mar 17, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
--- effort-missing
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Haiku 3.5 │ ⚡️ 20k/200k (▓░░░░░░░░░ 10%) · Cache 50% 60:00 · 📊 5h: ▓▓░░░░░░░░ 20% @17:46 · 7d: ▓▓▓▓▓░░░░░ 50% @Mar 24, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
--- one-million
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Opus 4.5 · 🧠 high │ ⚡️ 250k/1.0m (▓▓░░░░░░░░ 25%) · Cache 0% 60:00 · 📊 5h: ▓▓░░░░░░░░ 20% @17:46 · 7d: ▓▓▓▓▓░░░░░ 50% @Mar 24, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
--- multiple-weekly
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Fable 5 · 🧠 med │ ⚡️ 50k/200k (▓▓░░░░░░░░ 25%) · Cache 0% 60:00 · 📊 5h: ▓▓░░░░░░░░ 20% @17:46 · 7d: ▓▓▓▓▓░░░░░ 50% @Mar 24, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
--- wrapped
📁 mock-project › 🌿 integration [S1|W1] │ 🤖 Claude Enterprise Extremely Long Model Name · 🧠 xhigh
└─ ⚡️ 600k/1.0m (▓▓▓▓▓▓░░░░ 60%) · Cache 17% 60:00 · 📊 5h: ▓▓░░░░░░░░ 20% @17:46 · 7d: ▓▓▓▓▓░░░░░ 50% @Mar 24, 17:46 · Other: ▓▓▓▓▓▓▓▓▓░ 99% @Mar 24, 17:46 · Fable: ▓▓▓░░░░░░░ 30% @Mar 24, 17:46
"###);

        Ok(())
    }

    #[test]
    fn renders_with_unsafe_cache_directory_without_failing() {
        let app = StatusLineApp::new(
            MockGit,
            MockCredentials,
            MockHttp,
            CacheDir::Unsafe,
            FixedClock,
            UtcOffset,
            Config::from_values(Some("bar"), Some("2"), Some("1000")),
        );

        let rendered =
            strip_ansi(&app.render_input(include_str!("../tests/fixtures/status-input.json")));

        assert!(rendered.contains("📁 mock-project › 🌿 integration [S1|W1]"));
        assert!(rendered.contains("📊 5h: ▓▓░░░░░░░░ 20%"));
    }

    #[test]
    fn falls_back_from_empty_session_id_to_cwd_then_no_cwd() -> Result<(), serde_json::Error> {
        let cwd_fallback = crate::input::parse(r#"{"session_id":"","cwd":"/work/project"}"#)?;
        let no_cwd_fallback = crate::input::parse("{}")?;

        assert_eq!(session_key(&cwd_fallback), "/work/project");
        assert_eq!(session_key(&no_cwd_fallback), "no-cwd");

        Ok(())
    }
}
