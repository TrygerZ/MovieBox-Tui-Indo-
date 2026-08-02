use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub fn width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}

pub fn remove_last_grapheme(value: &mut String) {
    if let Some((index, _)) = value.grapheme_indices(true).next_back() {
        value.truncate(index);
    }
}

/// Strip C0/C1 control bytes and ESC sequences that a terminal would
/// interpret as commands (clear screen, OSC-52 clipboard, cursor moves).
/// Keeps `\n`, `\t`, `\r` so multiline layout still works. Applied to all
/// network-derived strings before they enter the render pipeline.
pub fn sanitize_terminal_text(value: &str) -> String {
    value
        .chars()
        .filter(|&c| {
            let cp = c as u32;
            // Drop C0 (except \t 0x09, \n 0x0a, \r 0x0d) — covers ESC (0x1b),
            // BEL (0x07) — and the C1 range (CSI 0x9b, OSC 0x9d, ...).
            (cp > 0x1f || matches!(cp, 0x09 | 0x0a | 0x0d)) && !(0x80..=0x9f).contains(&cp)
        })
        .collect()
}

pub fn truncate_width(value: &str, max_width: usize) -> String {
    if width(value) <= max_width {
        return value.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let content_width = max_width - 3;
    let mut output = String::new();
    let mut used = 0;
    for grapheme in value.graphemes(true) {
        let grapheme_width = width(grapheme);
        if used + grapheme_width > content_width {
            break;
        }
        output.push_str(grapheme);
        used += grapheme_width;
    }
    output.push_str("...");
    output
}

pub fn truncate_middle_width(value: &str, max_width: usize) -> String {
    if width(value) <= max_width {
        return value.to_string();
    }
    if max_width == 0 {
        return String::new();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let content_width = max_width - 1;
    let start_width = content_width.div_ceil(2);
    let end_width = content_width - start_width;

    let mut start = String::new();
    let mut used = 0;
    for grapheme in value.graphemes(true) {
        let grapheme_width = width(grapheme);
        if used + grapheme_width > start_width {
            break;
        }
        start.push_str(grapheme);
        used += grapheme_width;
    }

    let mut end = Vec::new();
    used = 0;
    for grapheme in value.graphemes(true).rev() {
        let grapheme_width = width(grapheme);
        if used + grapheme_width > end_width {
            break;
        }
        end.push(grapheme);
        used += grapheme_width;
    }
    end.reverse();

    format!("{start}…{}", end.concat())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_esc_and_control_bytes() {
        // ESC (0x1b), BEL (0x07), CSI (0x9b), OSC (0x9d), other C0 and C1.
        assert_eq!(sanitize_terminal_text("a\x1bb"), "ab");
        assert_eq!(sanitize_terminal_text("a\x07b"), "ab");
        assert_eq!(sanitize_terminal_text("a\u{9b}b"), "ab");
        assert_eq!(sanitize_terminal_text("a\u{9d}b"), "ab");
        assert_eq!(sanitize_terminal_text("a\x00b\x08c\x0cd\x1fe"), "abcde");
        assert_eq!(sanitize_terminal_text("a\u{80}b\u{9f}c"), "abc");
        assert_eq!(
            sanitize_terminal_text("\x1b[2J\x1b]52;c;test\x07"),
            "[2J]52;c;test"
        );
    }

    #[test]
    fn sanitize_keeps_newline_tab_cr_and_unicode() {
        assert_eq!(sanitize_terminal_text("a\nb\tc\rd"), "a\nb\tc\rd");
        assert_eq!(sanitize_terminal_text("ドラゴン (2024)"), "ドラゴン (2024)");
    }
}
