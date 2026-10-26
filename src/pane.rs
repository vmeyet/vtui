//! The frame every pane and modal is drawn in, so focus reads the same everywhere.
use crate::theme::Theme;
use ratatui::style::{Modifier, Style};
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
