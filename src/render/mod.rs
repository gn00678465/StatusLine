//! Assembles the rendered status line.

pub mod blocks;
pub mod color;
pub mod limits;
pub mod meter;

use crate::gitstatus::GitStatus;
use crate::ttl::CacheTtlStatus;
use crate::width::wrap_status_line;

use self::blocks::{render_cache, render_context, render_model, render_workspace, ContextUsage};
use self::color::{DIM, RESET};
use self::meter::MeterStyle;

pub struct RenderContext<'a> {
    pub cwd: Option<&'a str>,
    pub git_status: &'a GitStatus,
    pub model_name: &'a str,
    pub effort_level: Option<&'a str>,
    pub context_usage: ContextUsage,
    pub cache_status: CacheTtlStatus,
    pub limits: &'a str,
    pub meter_style: MeterStyle,
    pub columns: usize,
}

pub fn render(context: RenderContext<'_>) -> String {
    let workspace = render_workspace(context.cwd, context.git_status);
    let model = render_model(context.model_name, context.effort_level);
    let header = match workspace {
        Some(workspace) => format!("{workspace} {DIM}│{RESET} {model}"),
        None => model,
    };
    let context_block = render_context(context.context_usage, context.meter_style);
    let cache_block = render_cache(context.cache_status);
    let mut detail_blocks = vec![context_block];
    if !cache_block.is_empty() {
        detail_blocks.push(cache_block);
    }
    if !context.limits.is_empty() {
        detail_blocks.push(context.limits.to_owned());
    }
    let detail_block = detail_blocks.join(&format!(" {DIM}·{RESET} "));
    let wrapped = wrap_status_line(&header, &detail_block, context.columns);

    if wrapped.contains('\n') {
        format!("{header}\n{DIM}└─{RESET} {detail_block}")
    } else {
        format!("{header} {DIM}│{RESET} {detail_block}")
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use crate::gitstatus::GitStatus;
    use crate::render::blocks::ContextUsage;
    use crate::render::meter::MeterStyle;
    use crate::ttl::{CacheTtlStatus, TtlColor, TtlDisplay};
    use crate::width::strip_ansi;

    use super::{render, RenderContext};

    #[test]
    fn snapshots_workspace_with_git_effort_and_context() {
        let git_status = GitStatus {
            is_repository: true,
            branch: Some("main".to_owned()),
            staged: 1,
            unstaged: 2,
            conflicted: 3,
        };
        let rendered = render(RenderContext {
            cwd: Some("/work/project"),
            git_status: &git_status,
            model_name: "Sonnet",
            effort_level: Some("xhigh"),
            context_usage: ContextUsage {
                input_tokens: 10_000,
                cache_creation_tokens: 5_000,
                cache_read_tokens: 35_000,
                context_window_size: 200_000,
                official_percentage: Some(25.0),
            },
            cache_status: CacheTtlStatus {
                hit_rate: Some(50),
                ttl: Some(TtlDisplay::Countdown {
                    remaining_seconds: 3_600,
                    color: TtlColor::Green,
                }),
            },
            limits: "",
            meter_style: MeterStyle::Bar,
            columns: 100,
        });

        let plain = strip_ansi(&rendered);
        assert_snapshot!(
            &plain,
            @r###"
📁 project › 🌿 main [S1|W2|C3] │ 🤖 Sonnet · 🧠 xhigh
└─ ⚡️ 50k/200k (▓▓░░░░░░░░ 25%) · Cache 50% 60:00
"###
        );
    }

    #[test]
    fn snapshots_workspace_without_git_or_effort() {
        let git_status = GitStatus::default();
        let rendered = render(RenderContext {
            cwd: Some(r"C:\work\project"),
            git_status: &git_status,
            model_name: "Haiku",
            effort_level: None,
            context_usage: ContextUsage {
                input_tokens: 0,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                context_window_size: 200_000,
                official_percentage: None,
            },
            cache_status: CacheTtlStatus {
                hit_rate: None,
                ttl: None,
            },
            limits: "",
            meter_style: MeterStyle::Dots,
            columns: 100,
        });

        let plain = strip_ansi(&rendered);
        assert_snapshot!(&plain, @r###"📁 project │ 🤖 Haiku │ ⚡️ 0/200k (○○○○○○○○○○ 0%)"###);
    }

    #[test]
    fn wraps_the_context_block_using_ansi_aware_display_width() {
        let git_status = GitStatus::default();
        let rendered = render(RenderContext {
            cwd: Some("/work/project"),
            git_status: &git_status,
            model_name: "Haiku",
            effort_level: None,
            context_usage: ContextUsage {
                input_tokens: 0,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                context_window_size: 200_000,
                official_percentage: None,
            },
            cache_status: CacheTtlStatus {
                hit_rate: None,
                ttl: None,
            },
            limits: "",
            meter_style: MeterStyle::Dots,
            columns: 1,
        });

        assert_eq!(
            strip_ansi(&rendered),
            "📁 project │ 🤖 Haiku\n└─ ⚡️ 0/200k (○○○○○○○○○○ 0%)"
        );
    }

    #[test]
    fn renders_rate_limits_after_context_and_cache_blocks() {
        let git_status = GitStatus::default();
        let rendered = render(RenderContext {
            cwd: None,
            git_status: &git_status,
            model_name: "Haiku",
            effort_level: None,
            context_usage: ContextUsage {
                input_tokens: 0,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                context_window_size: 200_000,
                official_percentage: None,
            },
            cache_status: CacheTtlStatus {
                hit_rate: None,
                ttl: None,
            },
            limits: "📊 5h: - · 7d: -",
            meter_style: MeterStyle::Bar,
            columns: 100,
        });

        assert_eq!(
            strip_ansi(&rendered),
            "🤖 Haiku │ ⚡️ 0/200k (░░░░░░░░░░ 0%) · 📊 5h: - · 7d: -"
        );
    }
}
