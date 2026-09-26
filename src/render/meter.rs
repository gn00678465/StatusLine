//! Renders usage meters.

use super::color::{DIM, DIM_OFF, RESET};
use super::theme::{Rgb, Theme};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeterStyle {
    Bar,
    Dots,
}

impl MeterStyle {
    pub fn glyphs(self) -> (&'static str, &'static str) {
        match self {
            Self::Bar => ("▓", "░"),
            Self::Dots => ("●", "○"),
        }
    }

    pub fn ladder(self) -> [Rgb; 4] {
        match self {
            Self::Bar => [Rgb::GREEN, Rgb::YELLOW, Rgb::ORANGE, Rgb::RED],
            Self::Dots => [Rgb::GREEN, Rgb::ORANGE, Rgb::YELLOW, Rgb::RED],
        }
    }
}

pub fn render_meter(percentage: i64, theme: &Theme) -> String {
    let meter = &theme.meter;
    let percentage = percentage.clamp(0, 100);
    let filled = percentage as usize * meter.width / 100;
    let empty_cell = match meter.style {
        MeterStyle::Bar => meter.empty.clone(),
        MeterStyle::Dots => format!("{DIM}{}{DIM_OFF}", meter.empty),
    };
    let cells = format!(
        "{}{}",
        meter.filled.repeat(filled),
        empty_cell.repeat(meter.width - filled)
    );
    let color = theme.colors.level_color(percentage, meter.style.ladder());
    let label = if meter.show_percentage {
        format!(" {percentage}%")
    } else {
        String::new()
    };

    format!("{color}{cells}{label}{RESET}")
}

#[cfg(test)]
mod tests {
    use crate::render::color::{DIM, DIM_OFF, GREEN, ORANGE, RED, RESET, YELLOW};
    use crate::width::strip_ansi;

    use crate::render::theme::{Rgb, Theme};

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
            let rendered = render_meter(percentage, &Theme::new(style));

            assert!(rendered.starts_with(color));
            assert!(rendered.ends_with(RESET));
            assert_eq!(strip_ansi(&rendered), expected_plain);
        }

        assert_eq!(
            render_meter(0, &Theme::new(MeterStyle::Dots)),
            format!(
                "{GREEN}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF}{DIM}○{DIM_OFF} 0%{RESET}"
            )
        );
    }

    #[test]
    fn clamps_meter_percentages_before_rendering() {
        assert_eq!(
            render_meter(-1, &Theme::new(MeterStyle::Bar)),
            render_meter(0, &Theme::new(MeterStyle::Bar))
        );
        assert_eq!(
            render_meter(101, &Theme::new(MeterStyle::Dots)),
            render_meter(100, &Theme::new(MeterStyle::Dots))
        );
    }

    #[test]
    fn applies_meter_width_glyphs_and_percentage_knobs() {
        let mut theme = Theme::new(MeterStyle::Bar);
        theme.meter.width = 4;
        theme.meter.filled = "#".to_owned();
        theme.meter.empty = "-".to_owned();
        assert_eq!(strip_ansi(&render_meter(50, &theme)), "##-- 50%");

        theme.meter.show_percentage = false;
        assert_eq!(strip_ansi(&render_meter(50, &theme)), "##--");

        let mut dots = Theme::new(MeterStyle::Dots);
        dots.meter.width = 2;
        dots.meter.empty = "_".to_owned();
        assert_eq!(
            render_meter(50, &dots),
            format!("{ORANGE}●{DIM}_{DIM_OFF} 50%{RESET}")
        );
    }

    #[test]
    fn colors_meters_from_custom_levels_and_thresholds() {
        let mut theme = Theme::new(MeterStyle::Dots);
        theme.colors.thresholds = [10, 20, 30];
        theme.colors.levels = Some([Rgb(1, 1, 1), Rgb(2, 2, 2), Rgb(3, 3, 3), Rgb(4, 4, 4)]);

        assert!(render_meter(9, &theme).starts_with("\u{1b}[38;2;1;1;1m"));
        assert!(render_meter(25, &theme).starts_with("\u{1b}[38;2;3;3;3m"));

        theme.colors.levels = None;
        assert!(render_meter(25, &theme).starts_with(YELLOW));
    }
}
