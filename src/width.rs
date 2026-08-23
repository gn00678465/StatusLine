//! Measures rendered terminal display width.

use unicode_width::UnicodeWidthChar;

const MAIN_SEPARATOR: &str = " │ ";
const WRAP_PREFIX: &str = "└─ ";

pub fn strip_ansi(input: &str) -> String {
    let mut stripped = String::with_capacity(input.len());
    let mut characters = input.chars();

    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            stripped.push(character);
            continue;
        }

        if characters.next() == Some('[') {
            for sequence_character in characters.by_ref() {
                if matches!(sequence_character, '\u{0040}'..='\u{007E}') {
                    break;
                }
            }
        }
    }

    stripped
}

pub fn display_width(input: &str) -> usize {
    strip_ansi(input).chars().map(character_display_width).sum()
}

pub fn wrap_status_line(out: &str, limit: &str, columns: usize) -> String {
    if limit.is_empty() {
        return out.to_owned();
    }

    let inline = format!("{out}{MAIN_SEPARATOR}{limit}");
    if display_width(&inline) > columns {
        format!("{out}\n{WRAP_PREFIX}{limit}")
    } else {
        inline
    }
}

fn character_display_width(character: char) -> usize {
    if is_zero_width(character) {
        0
    } else if character == '\u{26A1}' || matches!(character, '\u{1F300}'..='\u{1FAFF}') {
        2
    } else {
        UnicodeWidthChar::width(character).into_iter().sum()
    }
}

fn is_zero_width(character: char) -> bool {
    matches!(
        character,
        '\u{0300}'..='\u{036F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FE00}'..='\u{FE0F}'
    )
}

#[cfg(test)]
mod tests {
    use super::{display_width, strip_ansi, wrap_status_line};

    #[test]
    fn strips_csi_ansi_sequences() {
        let colored = "\u{1b}[38;2;255;170;80m📁 project\u{1b}[0m";

        assert_eq!(strip_ansi(colored), "📁 project");
    }

    #[test]
    fn assigns_shell_widths_to_statusline_glyphs() {
        let glyph_widths = [
            ("📁", 2),
            ("⚡", 2),
            ("⚡️", 2),
            ("🤖", 2),
            ("🌿", 2),
            ("🧠", 2),
            ("📊", 2),
            ("·", 1),
            ("›", 1),
            ("│", 1),
            ("└─", 2),
            ("▓", 1),
            ("░", 1),
            ("●", 1),
            ("○", 1),
        ];

        for (glyph, expected_width) in glyph_widths {
            assert_eq!(display_width(glyph), expected_width, "{glyph}");
        }
    }

    #[test]
    fn applies_shell_width_overrides_for_emoji_and_zero_width_codepoints() {
        assert_eq!(display_width("\u{1F300}\u{1FAFF}"), 4);

        let variation_selectors: String = (0xFE00..=0xFE0F).filter_map(char::from_u32).collect();
        assert_eq!(display_width(&variation_selectors), 0);
        assert_eq!(display_width("A\u{0300}\u{200B}\u{202E}\u{2066}B"), 2);
    }

    #[test]
    fn measures_mixed_cjk_and_ansi_content_in_terminal_columns() {
        let colored = "\u{1b}[38;2;255;170;80mRust 中文 ⚡️\u{1b}[0m";

        assert_eq!(display_width(colored), 12);
    }

    #[test]
    fn keeps_an_exact_fit_on_one_line_and_wraps_one_column_short() {
        let out = "📁 repo";
        let limit = "⚡️ 50k/200k";

        assert_eq!(wrap_status_line(out, limit, 21), "📁 repo │ ⚡️ 50k/200k");
        assert_eq!(wrap_status_line(out, limit, 20), "📁 repo\n└─ ⚡️ 50k/200k");
    }
}
