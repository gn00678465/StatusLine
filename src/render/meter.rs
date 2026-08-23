//! Renders usage meters.

use super::color::{DIM, DIM_OFF, GREEN, ORANGE, RED, RESET, YELLOW};

const METER_WIDTH: usize = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeterStyle {
    Bar,
    Dots,
}

pub fn render_meter(percentage: i64, style: MeterStyle) -> String {
    let percentage = percentage.clamp(0, 100);
    let filled = percentage as usize * METER_WIDTH / 100;
    let cells = match style {
        MeterStyle::Bar => render_bar(filled),
        MeterStyle::Dots => render_dots(filled),
    };
    let color = meter_color(percentage, style);

    format!("{color}{cells} {percentage}%{RESET}")
}

fn meter_color(percentage: i64, style: MeterStyle) -> &'static str {
    match (style, percentage) {
        (_, 90..=100) => RED,
        (MeterStyle::Bar, 70..=89) => ORANGE,
        (MeterStyle::Bar, 50..=69) => YELLOW,
        (MeterStyle::Dots, 70..=89) => YELLOW,
        (MeterStyle::Dots, 50..=69) => ORANGE,
        _ => GREEN,
    }
}

fn render_bar(filled: usize) -> String {
    format!("{}{}", "▓".repeat(filled), "░".repeat(METER_WIDTH - filled))
}

fn render_dots(filled: usize) -> String {
    let mut dots = String::new();

    for index in 0..METER_WIDTH {
        if index < filled {
            dots.push('●');
        } else {
            dots.push_str(DIM);
            dots.push('○');
            dots.push_str(DIM_OFF);
        }
    }

    dots
}

#[cfg(test)]
mod tests {
    use crate::render::color::{DIM, DIM_OFF, GREEN, ORANGE, RED, RESET, YELLOW};
    use crate::width::strip_ansi;

    use super::{render_meter, MeterStyle};

    #[test]
    fn renders_ten_cells_with_the_shell_color_ladders() {
        let cases = [
            (MeterStyle::Bar, 0, GREEN, "░░░░░░░░░░ 0%"),
            (MeterStyle::Bar, 50, YELLOW, "▓▓▓▓▓░░░░░ 50%"),
            (MeterStyle::Bar, 70, ORANGE, "▓▓▓▓▓▓▓░░░ 70%"),
            (MeterStyle::Bar, 90, RED, "▓▓▓▓▓▓▓▓▓░ 90%"),
            (MeterStyle::Dots, 0, GREEN, "○○○○○○○○○○ 0%"),
            (MeterStyle::Dots, 50, ORANGE, "●●●●●○○○○○ 50%"),
            (MeterStyle::Dots, 70, YELLOW, "●●●●●●●○○○ 70%"),
            (MeterStyle::Dots, 90, RED, "●●●●●●●●●○ 90%"),
        ];

        for (style, percentage, color, expected_plain) in cases {
            let rendered = render_meter(percentage, style);

            assert!(rendered.starts_with(color));
            assert!(rendered.ends_with(RESET));
            assert_eq!(strip_ansi(&rendered), expected_plain);
        }

        assert_eq!(
            render_meter(0, MeterStyle::Dots),
            format!(
                "{GREEN}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF} 0%{RESET}"
            )
        );
    }

    #[test]
    fn clamps_meter_percentages_before_rendering() {
        assert_eq!(
            render_meter(-1, MeterStyle::Bar),
            render_meter(0, MeterStyle::Bar)
        );
        assert_eq!(
            render_meter(101, MeterStyle::Dots),
            render_meter(100, MeterStyle::Dots)
        );
    }
}
