//! The palette popup: the prompt with its mode, then the things to pick or the commands that match what is typed.
use super::{MAX_SHOWN, Mode, Palette, Verb};
use crate::pane;
use crate::text::truncate;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use unicode_width::UnicodeWidthStr;

/// The frame of the popup and the lines that do not depend on its width.
#[derive(Debug)]
pub struct Popup<'a> {
    /// The frame's title.
    pub title: &'a str,
    /// The first line, from [`prompt`].
    pub prompt: Line<'static>,
    /// The keys that work here, faded under the list.
    pub footer: &'a str,
}

/// One row of the list to pick from.
#[derive(Debug)]
pub struct Pick<'a> {
    /// What kind of thing it is, before the label.
    pub icon: &'static str,
    /// The icon's colour.
    pub colour: Color,
    /// What it is, cut to the room left by `detail`.
    pub label: &'a str,
    /// Faded text after the label, such as an author; may be empty.
    pub detail: &'a str,
}

/// Draws the popup over `area`; `list` gets the columns a row has and returns the rows between prompt and footer.
/// On a short terminal the list loses its last rows; the prompt and the footer always show.
pub fn draw(f: &mut Frame<'_>, theme: &Theme, area: Rect, popup: Popup<'_>, list: impl FnOnce(usize) -> Vec<Line<'static>>) {
    let width = (area.width * 3 / 5).clamp(40.min(area.width), area.width);
    let height = (MAX_SHOWN as u16 + 5).min(area.height);
    let at = Rect { x: area.x + (area.width - width) / 2, y: area.y + area.height.saturating_sub(height) / 3, width, height };
    let block = pane::framed(theme, popup.title, true);
    let inner = block.inner(at);
    let room = inner.width.saturating_sub(4) as usize;
    let rows = inner.height.saturating_sub(3) as usize;
    let mut lines = vec![popup.prompt, Line::default()];
    lines.extend(list(room).into_iter().take(rows));
    lines.push(Line::from(Span::styled(popup.footer.to_owned(), Style::default().fg(theme.faded))));
    f.render_widget(Clear, at);
    f.render_widget(Paragraph::new(lines).block(block).style(Style::default().bg(theme.surface)), at);
}

/// `› text▌`, or `sign text▌` in a mode with a sign, then the options tab is cycling through or the ghost of a
/// completion; `completions` gives the candidates for the line, and is only asked in a mode that takes commands.
pub fn prompt<M: Mode>(theme: &Theme, palette: &Palette<M>, sign: &str, completions: impl FnOnce(&str) -> Vec<String>) -> Line<'static> {
    let mut spans = vec![
        Span::styled(
            format!("{} ", if sign.is_empty() { "›" } else { sign }),
            Style::default().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Span::raw(palette.input.clone()),
        Span::styled("▌", Style::default().fg(theme.accent)),
    ];
    if palette.mode.takes_commands() {
        let candidates = completions(&palette.input);
        match (palette.hint(), palette.ghost(&candidates)) {
            (Some(hint), _) => spans.push(Span::styled(format!("   {hint}"), Style::default().fg(theme.muted))),
            (None, Some(ghost)) => spans.push(Span::styled(ghost, Style::default().fg(theme.faded))),
            (None, None) => {}
        }
    }
    Line::from(spans)
}

/// The rows to pick from, the highlighted one marked with the bar; `no match` when there are none.
pub fn pick_lines(theme: &Theme, picks: &[Pick<'_>], selected: usize, room: usize) -> Vec<Line<'static>> {
    if picks.is_empty() {
        return vec![Line::from(Span::styled("  no match", Style::default().fg(theme.muted)))];
    }
    picks.iter().enumerate().map(|(i, pick)| pick_line(theme, pick, i == selected, room)).collect()
}

fn pick_line(theme: &Theme, pick: &Pick<'_>, selected: bool, room: usize) -> Line<'static> {
    let detail = if pick.detail.is_empty() { String::new() } else { format!("  {}", pick.detail) };
    let label = truncate(pick.label, room.saturating_sub(detail.width()));
    let text = if selected { Style::default().add_modifier(Modifier::BOLD) } else { Style::default() };
    Line::from(vec![
        Span::styled(if selected { "▎" } else { " " }, Style::default().fg(theme.accent)),
        Span::styled(format!("{} ", pick.icon), Style::default().fg(pick.colour)),
        Span::styled(label, text),
        Span::styled(detail, Style::default().fg(theme.faded)),
    ])
}

/// The commands that match the first word typed, each with what it does; the highlighted one while the line
/// still names a command.
pub fn verb_lines<M: Mode>(theme: &Theme, palette: &Palette<M>, verbs: &[Verb], room: usize) -> Vec<Line<'static>> {
    let naming = palette.naming_command();
    verbs
        .iter()
        .enumerate()
        .map(|(i, verb)| {
            let here = naming && i == palette.selected;
            let line = Line::from(vec![
                Span::styled(format!("{}{:<9}", if here { "▸ " } else { "  " }, verb.word), Style::default().fg(theme.accent)),
                Span::styled(truncate(verb.does, room.saturating_sub(11)), Style::default().fg(theme.muted)),
            ]);
            if here { line.style(Style::default().bg(theme.highlight.unwrap_or(theme.base))) } else { line }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn drawn(width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("a test backend");
        let popup = Popup { title: "acme/widgets", prompt: Line::from("› nina"), footer: "enter open" };
        let rows = |_| (0..MAX_SHOWN).map(|i| Line::from(format!("row {i}"))).collect();
        terminal.draw(|f| draw(f, &Theme::default(), f.area(), popup, rows)).expect("a draw");
        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .chunks(buffer.area.width as usize)
            .map(|row| row.iter().map(ratatui::buffer::Cell::symbol).collect::<String>() + "\n")
            .collect()
    }

    #[test]
    fn the_footer_shows_under_a_full_list() {
        let screen = drawn(60, 30);
        assert!(screen.contains("enter open"));
        assert!(screen.contains(&format!("row {}", MAX_SHOWN - 1)));
    }

    #[test]
    fn a_short_terminal_cuts_the_list_and_keeps_the_prompt_and_footer() {
        let screen = drawn(60, 8);
        assert!(screen.contains("› nina"));
        assert!(screen.contains("enter open"));
        assert!(screen.contains("row 2"));
        assert!(!screen.contains("row 3"));
    }
}
