use ratatui::{
    Frame,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::{text::TextInputBuffer, theme::Theme};

pub fn render_single_line_input(
    frame: &mut Frame,
    area: Rect,
    label: &str,
    buffer: &TextInputBuffer,
    theme: &Theme,
    basic_terminal: bool,
) {
    let segments = buffer.graphemes();
    let cursor = buffer.cursor();
    let prompt_symbol = if basic_terminal { " > " } else { " ❯ " };
    let prompt_width = crate::tui::text::width(prompt_symbol);
    let available_width = (area.width as usize)
        .saturating_sub(prompt_width + 1)
        .max(1);

    let cursor_grapheme = if cursor < segments.len() {
        segments[cursor]
    } else {
        " "
    };
    let cursor_w = crate::tui::text::width(cursor_grapheme).max(1);

    let after_reserve = if cursor.saturating_add(1) < segments.len() {
        3
    } else {
        0
    };
    let max_before_w = available_width.saturating_sub(cursor_w + after_reserve);
    let mut start = cursor;
    let mut current_before_w = 0;
    while start > 0 {
        let prev_gw = crate::tui::text::width(segments[start - 1]);
        if current_before_w + prev_gw > max_before_w {
            break;
        }
        current_before_w += prev_gw;
        start -= 1;
    }

    let before_cursor = if start > 0 {
        let budget = max_before_w.saturating_sub(3);
        let mut adj_start = cursor;
        let mut adj_w = 0;
        while adj_start > 0 {
            let gw = crate::tui::text::width(segments[adj_start - 1]);
            if adj_w + gw > budget {
                break;
            }
            adj_w += gw;
            adj_start -= 1;
        }
        format!("...{}", segments[adj_start..cursor].concat())
    } else {
        segments[..cursor].concat()
    };

    let cursor_char = cursor_grapheme.to_string();
    let before_w = crate::tui::text::width(&before_cursor);
    let remaining_after_w = available_width.saturating_sub(before_w + cursor_w);
    let mut end = cursor.saturating_add(1).min(segments.len());
    let mut current_after_w = 0;
    while end < segments.len() {
        let next_gw = crate::tui::text::width(segments[end]);
        if current_after_w + next_gw > remaining_after_w {
            break;
        }
        current_after_w += next_gw;
        end += 1;
    }

    let after_cursor = if end < segments.len() {
        if remaining_after_w >= 3 {
            let budget = remaining_after_w.saturating_sub(3);
            let mut adj_end = cursor.saturating_add(1).min(segments.len());
            let mut adj_w = 0;
            while adj_end < segments.len() {
                let gw = crate::tui::text::width(segments[adj_end]);
                if adj_w + gw > budget {
                    break;
                }
                adj_w += gw;
                adj_end += 1;
            }
            format!(
                "{}...",
                segments[cursor.saturating_add(1).min(segments.len())..adj_end].concat()
            )
        } else {
            segments[cursor.saturating_add(1).min(segments.len())..end].concat()
        }
    } else {
        segments[cursor.saturating_add(1).min(segments.len())..end].concat()
    };
    let lines = vec![
        Line::from(vec![Span::raw(" "), Span::styled(label, theme.sapphire)]),
        Line::from(vec![
            Span::styled(prompt_symbol, theme.sapphire),
            Span::styled(before_cursor, theme.text),
            Span::styled(cursor_char, theme.text.add_modifier(Modifier::REVERSED)),
            Span::styled(after_cursor, theme.text),
        ]),
    ];

    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn test_render_single_line_input_empty() {
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let buffer = TextInputBuffer::new();
        let theme = Theme::default();

        terminal
            .draw(|f| {
                render_single_line_input(
                    f,
                    Rect::new(0, 0, 80, 3),
                    "Enter URL:",
                    &buffer,
                    &theme,
                    false,
                );
            })
            .unwrap();
    }

    #[test]
    fn test_render_single_line_input_with_content() {
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut buffer = TextInputBuffer::from_str("https://example.com/playlist.m3u8");
        buffer.move_home();
        let theme = Theme::default();

        terminal
            .draw(|f| {
                render_single_line_input(
                    f,
                    Rect::new(0, 0, 40, 3),
                    "Enter URL:",
                    &buffer,
                    &theme,
                    true,
                );
            })
            .unwrap();
    }
    #[test]
    fn test_render_single_line_input_cjk_no_wrap() {
        let backend = TestBackend::new(30, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let buffer = TextInputBuffer::from_str("https://example.com/电影/电视/动漫/超高清.m3u8");
        let theme = Theme::default();

        terminal
            .draw(|f| {
                render_single_line_input(
                    f,
                    Rect::new(0, 0, 30, 3),
                    "Enter URL:",
                    &buffer,
                    &theme,
                    false,
                );
            })
            .unwrap();
    }
    #[test]
    fn test_render_single_line_input_truncation_no_double_ellipsis() {
        let backend = TestBackend::new(20, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let buffer =
            TextInputBuffer::from_str("https://very-long-domain-name.example.com/stream.m3u8");
        let theme = Theme::default();
        terminal
            .draw(|f| {
                render_single_line_input(f, Rect::new(0, 0, 20, 3), "URL:", &buffer, &theme, false);
            })
            .unwrap();
    }

    #[test]
    fn test_render_single_line_input_cursor_movement_no_line_overflow() {
        let backend = TestBackend::new(46, 5);
        let mut terminal = Terminal::new(backend).unwrap();
        let long_str = "a".repeat(80);
        let theme = Theme::default();

        for cursor_pos in 0..=80 {
            let mut buffer = TextInputBuffer::from_str(&long_str);
            for _ in 0..80 {
                buffer.move_left();
            }
            for _ in 0..cursor_pos {
                buffer.move_right();
            }
            terminal
                .draw(|f| {
                    render_single_line_input(
                        f,
                        Rect::new(0, 0, 46, 3),
                        "Enter Addon Manifest URL:",
                        &buffer,
                        &theme,
                        false,
                    );
                })
                .unwrap();

            let line_0: String = (0..46)
                .map(|x| terminal.backend().buffer().cell((x, 0)).unwrap().symbol())
                .collect();
            let line_1: String = (0..46)
                .map(|x| terminal.backend().buffer().cell((x, 1)).unwrap().symbol())
                .collect();
            let line_2: String = (0..46)
                .map(|x| terminal.backend().buffer().cell((x, 2)).unwrap().symbol())
                .collect();

            assert!(line_0.contains("Enter Addon Manifest URL:"));
            assert!(
                line_1.contains('❯'),
                "line 1 must contain prompt symbol at cursor {cursor_pos}: {line_1}"
            );
            assert!(
                line_1.contains('a') || line_1.contains('.'),
                "line 1 must contain input text at cursor {cursor_pos}: {line_1}"
            );
            assert_eq!(
                line_2.trim(),
                "",
                "line 2 must be empty and not wrapped at cursor {cursor_pos}: {line_2}"
            );
        }
    }
}
