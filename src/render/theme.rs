//! Holds the user-tunable appearance: layout, meter shape, and colors.

use std::fmt;

use super::meter::MeterStyle;

pub const DEFAULT_METER_WIDTH: usize = 10;
pub const MAX_METER_WIDTH: usize = 20;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Theme {
    pub layout: Layout,
    pub meter: MeterTheme,
    pub colors: Palette,
}

impl Theme {
    pub fn new(style: MeterStyle) -> Self {
        let (filled, empty) = style.glyphs();

        Self {
            layout: Layout::Auto,
            meter: MeterTheme {
                style,
                width: DEFAULT_METER_WIDTH,
                filled: filled.to_owned(),
                empty: empty.to_owned(),
                show_percentage: true,
                show_reset: true,
            },
            colors: Palette::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    #[default]
    Auto,
    Stacked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeterTheme {
    pub style: MeterStyle,
    pub width: usize,
    pub filled: String,
    pub empty: String,
    pub show_percentage: bool,
    pub show_reset: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Palette {
    pub folder: Rgb,
    pub branch: Rgb,
    pub model: Rgb,
    pub tokens: Rgb,
    pub levels: Option<[Rgb; 4]>,
    pub thresholds: [u8; 3],
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            folder: Rgb::CYAN,
            branch: Rgb::GREEN,
            model: Rgb::BLUE,
            tokens: Rgb::WHITE,
            levels: None,
            thresholds: [50, 70, 90],
        }
    }
}

impl Palette {
    /// `ladder` is the caller's default and applies only while `levels` is unset.
    pub fn level_color(&self, percentage: i64, ladder: [Rgb; 4]) -> Rgb {
        let level = self
            .thresholds
            .iter()
            .filter(|threshold| percentage >= i64::from(**threshold))
            .count();

        self.levels.unwrap_or(ladder)[level]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize)]
#[serde(try_from = "String")]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const BLUE: Self = Self(80, 180, 255);
    pub const ORANGE: Self = Self(255, 170, 80);
    pub const GREEN: Self = Self(100, 255, 100);
    pub const CYAN: Self = Self(100, 220, 255);
    pub const RED: Self = Self(255, 100, 100);
    pub const YELLOW: Self = Self(255, 230, 80);
    pub const WHITE: Self = Self(240, 240, 240);
}

impl fmt::Display for Rgb {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "\u{1b}[38;2;{};{};{}m", self.0, self.1, self.2)
    }
}

impl TryFrom<String> for Rgb {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        parse_hex(&value).ok_or_else(|| format!("{value:?} is not \"#RRGGBB\""))
    }
}

fn parse_hex(value: &str) -> Option<Rgb> {
    let hex = value.strip_prefix('#')?;
    // `from_str_radix` accepts a leading sign, so check the digits first.
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |start: usize| u8::from_str_radix(hex.get(start..start + 2)?, 16).ok();

    Some(Rgb(channel(0)?, channel(2)?, channel(4)?))
}

#[cfg(test)]
mod tests {
    use crate::render::color::{BLUE, CYAN, GREEN, ORANGE, RED, WHITE, YELLOW};

    use super::{Palette, Rgb};

    #[test]
    fn default_rgb_constants_match_the_ansi_palette() {
        let pairs = [
            (Rgb::BLUE, BLUE),
            (Rgb::ORANGE, ORANGE),
            (Rgb::GREEN, GREEN),
            (Rgb::CYAN, CYAN),
            (Rgb::RED, RED),
            (Rgb::YELLOW, YELLOW),
            (Rgb::WHITE, WHITE),
        ];

        for (rgb, ansi) in pairs {
            assert_eq!(rgb.to_string(), ansi);
        }
    }

    #[test]
    fn parses_only_strict_hash_rrggbb() {
        assert_eq!(Rgb::try_from("#0aFf10".to_owned()), Ok(Rgb(10, 255, 16)));

        for invalid in [
            "0aff10", "#0aff1", "#0aff100", "#0aff1g", "#+aff10", "#ｆｆ",
        ] {
            assert!(Rgb::try_from(invalid.to_owned()).is_err(), "{invalid}");
        }
    }

    #[test]
    fn picks_the_level_color_by_threshold_with_levels_overriding_the_ladder() {
        let ladder = [Rgb(0, 0, 0), Rgb(1, 1, 1), Rgb(2, 2, 2), Rgb(3, 3, 3)];
        let custom = Palette {
            thresholds: [10, 20, 30],
            ..Palette::default()
        };

        let picked: Vec<Rgb> = [9, 10, 19, 20, 29, 30]
            .into_iter()
            .map(|percentage| custom.level_color(percentage, ladder))
            .collect();
        assert_eq!(
            picked,
            [
                Rgb(0, 0, 0),
                Rgb(1, 1, 1),
                Rgb(1, 1, 1),
                Rgb(2, 2, 2),
                Rgb(2, 2, 2),
                Rgb(3, 3, 3)
            ]
        );

        let overridden = Palette {
            levels: Some([Rgb(9, 0, 0), Rgb(9, 1, 1), Rgb(9, 2, 2), Rgb(9, 3, 3)]),
            ..Palette::default()
        };
        assert_eq!(overridden.level_color(70, ladder), Rgb(9, 2, 2));
    }
}
