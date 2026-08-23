//! Renders individual status-line blocks.

use super::color::{BLUE, BRIGHT_RED, CYAN, DIM, GRAY, GREEN, ORANGE, RED, RESET, WHITE, YELLOW};
use super::meter::{render_meter, MeterStyle};
use crate::gitstatus::GitStatus;
use crate::ttl::{CacheTtlStatus, TtlColor, TtlDisplay};

pub fn format_tokens(tokens: u64) -> String {
    if tokens >= 1_000_000 {
        let mut whole = tokens / 1_000_000;
        let mut fraction = (tokens % 1_000_000 + 50_000) / 100_000;
        if fraction >= 10 {
            whole = whole.saturating_add(1);
            fraction = 0;
        }

        format!("{whole}.{fraction}m")
    } else if tokens >= 1_000 {
        format!("{}k", tokens.saturating_add(500) / 1_000)
    } else {
        tokens.to_string()
    }
}

pub fn render_model(model_name: &str, effort_level: Option<&str>) -> String {
    let mut block = format!("🤖 {BLUE}{model_name}{RESET}");

    if let Some(effort_level) = effort_level {
        let (color, label) = match effort_level {
            "low" => (DIM, "low"),
            "medium" => (YELLOW, "med"),
            "max" => (RED, "max"),
            "high" => (ORANGE, "high"),
            "xhigh" => (ORANGE, "xhigh"),
            _ => (ORANGE, effort_level),
        };
        block.push_str(&format!(" {DIM}·{RESET} 🧠 {color}{label}{RESET}"));
    }

    block
}

pub fn render_workspace(cwd: Option<&str>, git_status: &GitStatus) -> Option<String> {
    let cwd = cwd?;
    let normalized_cwd = cwd.replace('\\', "/");
    let display_directory: String = normalized_cwd.rsplit('/').take(1).collect();
    let mut block = format!("📁 {CYAN}{display_directory}{RESET}");

    if git_status.is_repository {
        if let Some(branch) = git_status.branch.as_deref() {
            block.push_str(&format!(" {DIM}›{RESET} 🌿 {GREEN}{branch}{RESET}"));

            let mut details = Vec::new();
            if git_status.staged > 0 {
                details.push(format!("{GREEN}S{}{RESET}", git_status.staged));
            }
            if git_status.unstaged > 0 {
                details.push(format!("{YELLOW}W{}{RESET}", git_status.unstaged));
            }
            if git_status.conflicted > 0 {
                details.push(format!("{RED}C{}{RESET}", git_status.conflicted));
            }

            if !details.is_empty() {
                block.push_str(&format!(
                    " {DIM}[{RESET}{}{DIM}]{RESET}",
                    details.join(&format!("{DIM}|{RESET}"))
                ));
            }
        }
    }

    Some(block)
}

#[derive(Clone, Copy, Debug)]
pub struct ContextUsage {
    pub input_tokens: u64,
    pub cache_creation_tokens: u64,
    pub cache_read_tokens: u64,
    pub context_window_size: u64,
    pub official_percentage: Option<f64>,
}

pub fn render_context(usage: ContextUsage, meter_style: MeterStyle) -> String {
    let current_tokens = u128::from(usage.input_tokens)
        + u128::from(usage.cache_creation_tokens)
        + u128::from(usage.cache_read_tokens);
    let used_tokens = format_tokens(current_tokens.min(u128::from(u64::MAX)) as u64);
    let total_tokens = format_tokens(usage.context_window_size);
    let percentage = context_percentage(usage, current_tokens);
    let meter = render_meter(i64::from(percentage), meter_style);

    format!("⚡️ {WHITE}{used_tokens}{RESET}{DIM}/{total_tokens}{RESET} {DIM}({meter}{DIM}){RESET}")
}

pub fn render_cache(status: CacheTtlStatus) -> String {
    if status.hit_rate.is_none() && status.ttl.is_none() {
        return String::new();
    }

    let mut block = format!("{DIM}Cache {RESET}");
    if let Some(hit_rate) = status.hit_rate {
        let hit_rate_color = if hit_rate >= 50 { GREEN } else { GRAY };
        block.push_str(&format!("{hit_rate_color}{hit_rate}%{RESET}"));
    }
    if let Some(ttl) = status.ttl {
        if status.hit_rate.is_some() {
            block.push(' ');
        }
        let (ttl_color, ttl_text) = match ttl {
            TtlDisplay::Countdown {
                remaining_seconds,
                color,
            } => (ttl_color(color), format_ttl(remaining_seconds)),
            TtlDisplay::Expired => (GRAY, "exp".to_owned()),
        };
        block.push_str(&format!("{ttl_color}{ttl_text}{RESET}"));
    }

    block
}

fn ttl_color(color: TtlColor) -> &'static str {
    match color {
        TtlColor::Green => GREEN,
        TtlColor::Yellow => YELLOW,
        TtlColor::Red => RED,
        TtlColor::BrightRed => BRIGHT_RED,
    }
}

fn format_ttl(remaining_seconds: u64) -> String {
    format!("{}:{:02}", remaining_seconds / 60, remaining_seconds % 60)
}

fn context_percentage(usage: ContextUsage, current_tokens: u128) -> u8 {
    match usage.official_percentage {
        Some(percentage) if percentage.is_finite() => rounded_percentage(percentage),
        Some(_) | None => fallback_context_percentage(current_tokens, usage.context_window_size),
    }
}

fn rounded_percentage(percentage: f64) -> u8 {
    let rounded = percentage.round();
    if rounded <= 0.0 {
        0
    } else if rounded >= 100.0 {
        100
    } else {
        rounded as u8
    }
}

fn fallback_context_percentage(current_tokens: u128, context_window_size: u64) -> u8 {
    if context_window_size == 0 {
        return 0;
    }

    let percentage = current_tokens * 100 / u128::from(context_window_size);
    percentage.min(100) as u8
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use crate::render::color::{BLUE, BRIGHT_RED, DIM, GRAY, ORANGE, RED, RESET, YELLOW};
    use crate::render::meter::MeterStyle;
    use crate::ttl::{CacheTtlStatus, TtlColor, TtlDisplay};
    use crate::width::strip_ansi;

    use super::{format_tokens, render_cache, render_context, render_model, ContextUsage};

    #[test]
    fn formats_token_counts_with_shell_rounding_boundaries() {
        let cases = [
            (999, "999"),
            (1_000, "1k"),
            (1_500, "2k"),
            (999_999, "1000k"),
            (1_000_000, "1.0m"),
            (1_050_000, "1.1m"),
            (1_950_000, "2.0m"),
        ];

        for (tokens, expected) in cases {
            assert_eq!(format_tokens(tokens), expected, "{tokens}");
        }
    }

    #[test]
    fn renders_all_effort_levels_and_omits_a_missing_effort() {
        let cases = [
            ("low", DIM, "low"),
            ("medium", YELLOW, "med"),
            ("high", ORANGE, "high"),
            ("xhigh", ORANGE, "xhigh"),
            ("max", RED, "max"),
        ];

        for (effort, color, label) in cases {
            assert_eq!(
                render_model("Sonnet", Some(effort)),
                format!("🤖 {BLUE}Sonnet{RESET} {DIM}·{RESET} 🧠 {color}{label}{RESET}")
            );
        }

        assert_eq!(
            render_model("Sonnet", None),
            format!("🤖 {BLUE}Sonnet{RESET}")
        );
    }

    #[test]
    fn prioritizes_rounded_official_context_percentage_then_falls_back_to_tokens() {
        let official = render_context(
            ContextUsage {
                input_tokens: 80,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                context_window_size: 1_000,
                official_percentage: Some(49.5),
            },
            MeterStyle::Bar,
        );
        let fallback = render_context(
            ContextUsage {
                input_tokens: 1_500,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
                context_window_size: 10_000,
                official_percentage: None,
            },
            MeterStyle::Bar,
        );

        assert_eq!(strip_ansi(&official), "⚡️ 80/1k (▓▓▓▓▓░░░░░ 50%)");
        assert_eq!(strip_ansi(&fallback), "⚡️ 2k/10k (▓░░░░░░░░░ 15%)");
    }

    #[test]
    fn snapshots_cache_blocks_for_ttl_states_and_last_hit_rate() {
        let fresh = render_cache(CacheTtlStatus {
            hit_rate: Some(50),
            ttl: Some(TtlDisplay::Countdown {
                remaining_seconds: 3_600,
                color: TtlColor::Green,
            }),
        });
        let warning = render_cache(CacheTtlStatus {
            hit_rate: Some(49),
            ttl: Some(TtlDisplay::Countdown {
                remaining_seconds: 2_400,
                color: TtlColor::Yellow,
            }),
        });
        let flashing = render_cache(CacheTtlStatus {
            hit_rate: Some(50),
            ttl: Some(TtlDisplay::Countdown {
                remaining_seconds: 300,
                color: TtlColor::BrightRed,
            }),
        });
        let expired = render_cache(CacheTtlStatus {
            hit_rate: Some(50),
            ttl: Some(TtlDisplay::Expired),
        });
        let hit_rate_only = render_cache(CacheTtlStatus {
            hit_rate: Some(49),
            ttl: None,
        });

        let plain = strip_ansi(&[fresh, warning, flashing, expired, hit_rate_only].join("\n"));
        assert_snapshot!(
            &plain,
            @r###"
Cache 50% 60:00
Cache 49% 40:00
Cache 50% 5:00
Cache 50% exp
Cache 49%
"###
        );
    }

    #[test]
    fn applies_threshold_and_flashing_colors_to_cache_fields() {
        let cache = render_cache(CacheTtlStatus {
            hit_rate: Some(49),
            ttl: Some(TtlDisplay::Countdown {
                remaining_seconds: 300,
                color: TtlColor::BrightRed,
            }),
        });

        assert_eq!(
            cache,
            format!("{DIM}Cache {RESET}{GRAY}49%{RESET} {BRIGHT_RED}5:00{RESET}")
        );
    }
}
