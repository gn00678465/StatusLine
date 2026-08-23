//! Defines ANSI color constants for rendering.

pub const BLUE: &str = "\u{1b}[38;2;80;180;255m";
pub const ORANGE: &str = "\u{1b}[38;2;255;170;80m";
pub const GREEN: &str = "\u{1b}[38;2;100;255;100m";
pub const CYAN: &str = "\u{1b}[38;2;100;220;255m";
pub const RED: &str = "\u{1b}[38;2;255;100;100m";
pub const YELLOW: &str = "\u{1b}[38;2;255;230;80m";
pub const WHITE: &str = "\u{1b}[38;2;240;240;240m";
pub const DIM: &str = "\u{1b}[2m";
pub const DIM_OFF: &str = "\u{1b}[22m";
pub const RESET: &str = "\u{1b}[0m";
pub const BRIGHT_RED: &str = "\u{1b}[38;2;255;50;50m";
pub const GRAY: &str = "\u{1b}[38;2;140;140;140m";

#[cfg(test)]
mod tests {
    use super::{
        BLUE, BRIGHT_RED, CYAN, DIM, DIM_OFF, GRAY, GREEN, ORANGE, RED, RESET, WHITE, YELLOW,
    };

    #[test]
    fn matches_the_shell_truecolor_palette_and_attributes() {
        assert_eq!(BLUE, "\u{1b}[38;2;80;180;255m");
        assert_eq!(ORANGE, "\u{1b}[38;2;255;170;80m");
        assert_eq!(GREEN, "\u{1b}[38;2;100;255;100m");
        assert_eq!(CYAN, "\u{1b}[38;2;100;220;255m");
        assert_eq!(RED, "\u{1b}[38;2;255;100;100m");
        assert_eq!(YELLOW, "\u{1b}[38;2;255;230;80m");
        assert_eq!(WHITE, "\u{1b}[38;2;240;240;240m");
        assert_eq!(BRIGHT_RED, "\u{1b}[38;2;255;50;50m");
        assert_eq!(GRAY, "\u{1b}[38;2;140;140;140m");
        assert_eq!(DIM, "\u{1b}[2m");
        assert_eq!(DIM_OFF, "\u{1b}[22m");
        assert_eq!(RESET, "\u{1b}[0m");
    }
}
