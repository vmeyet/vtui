//! The frame every pane and modal is drawn in, so focus reads the same everywhere.
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, BorderType, Padding};

/// A rounded frame titled `title`: the accent and the brighter border mark the pane that has the keys.
pub fn framed(theme: &Theme, title: &str, focused: bool) -> Block<'static> {
    let title_style =
        if focused { Style::default().fg(theme.accent).add_modifier(Modifier::BOLD) } else { Style::default().fg(theme.faded) };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(if focused { theme.border_focus } else { theme.border }))
        .title(Span::styled(format!(" {title} "), title_style))
        .padding(Padding::horizontal(1))
}

/// Dims what is already drawn in `area`, as behind a modal: its text takes `color` and loses bold, and its frames
/// take the quiet `border`. `color` is brighter than any frame, so fading a frame would light the unfocused pane's box
/// above the focused one's.
pub fn fade(f: &mut Frame<'_>, area: Rect, theme: &Theme, color: Color) {
    let buf = f.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                let frame = cell.fg == theme.border || cell.fg == theme.border_focus;
                cell.set_fg(if frame { theme.border } else { color });
                cell.modifier.remove(Modifier::BOLD);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::widgets::Paragraph;

    const FADED: Color = Color::Indexed(99);

    fn faded(draw: impl Fn(&mut Frame<'_>), area: Rect) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(12, 3)).expect("a test backend");
        let theme = Theme::default();
        terminal
            .draw(|f| {
                draw(f);
                fade(f, area, &theme, FADED);
            })
            .expect("a draw");
        terminal.backend().buffer().clone()
    }

    #[test]
    fn faded_text_takes_the_colour_and_loses_bold() {
        let bold = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
        let buf = faded(|f| f.render_widget(Paragraph::new(Span::styled("nina", bold)), f.area()), Rect::new(0, 0, 12, 3));
        assert_eq!(buf[(0, 0)].fg, FADED);
        assert!(!buf[(0, 0)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn faded_frames_all_take_the_quiet_border() {
        let theme = Theme::default();
        let buf = faded(|f| f.render_widget(framed(&theme, "acme/widgets", true), f.area()), Rect::new(0, 0, 12, 3));
        assert_eq!(buf[(0, 0)].fg, theme.border);
        assert_eq!(buf[(0, 2)].fg, theme.border);
    }

    #[test]
    fn cells_outside_the_area_stay_as_drawn() {
        let red = Style::default().fg(Color::Red);
        let buf = faded(|f| f.render_widget(Paragraph::new(Span::styled("nina", red)), f.area()), Rect::new(0, 1, 12, 2));
        assert_eq!(buf[(0, 0)].fg, Color::Red);
    }
}
