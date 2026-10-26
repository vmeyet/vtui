//! The `?` overlay: the keys the app hands it, grouped in sections, in one or two columns.
//! The app picks the sections; a second `?` usually asks for every key instead of the focused pane's.
use crate::pane;
use crate::text::truncate;
use crate::theme::Theme;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Padding, Paragraph};
use unicode_width::UnicodeWidthStr;

/// The overlay while open: how far it scrolled, and whether it lists every key or the focused pane's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Help {
    /// The first row shown.
    pub scroll: usize,
    /// Every key of the app, not only the focused pane's.
    pub every_key: bool,
}

/// A titled list of keys, drawn as one block of the overlay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    /// Drawn in faded capitals above the keys.
    pub title: &'static str,
    /// The keys in the order they are listed.
    pub rows: Vec<Row>,
}

/// One key of a section and what it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The keys as the reader presses them, already in the user's bindings.
    pub keys: String,
    /// What they do, cut to the column's width.
    pub does: &'static str,
}

/// Padding inside the frame: two columns on each side, one row above and below.
const PAD_X: u16 = 2;
const PAD_Y: u16 = 1;
/// Space between the two columns.
const GUTTER: usize = 6;
/// Space between a key and what it does.
const KEY_GAP: usize = 2;
/// From this overlay width the sections share two columns.
const TWO_COLUMNS_FROM: u16 = 100;

/// The last row the list can scroll to: its sections in one column, so scrolling never stops short.
pub fn last_row(sections: &[Section]) -> usize {
    column_height(&sizes(sections)).saturating_sub(1)
}

fn sizes(sections: &[Section]) -> Vec<usize> {
    sections.iter().map(|section| section.rows.len()).collect()
}

/// Rows a column of sections takes: a header per section, its keys, one blank row between sections.
fn column_height(sizes: &[usize]) -> usize {
    sizes.iter().map(|n| n + 1).sum::<usize>() + sizes.len().saturating_sub(1)
}

/// How many sections go in the left column so both columns end as close as possible; order is kept.
fn split(sizes: &[usize]) -> usize {
    (1..sizes.len()).min_by_key(|&at| column_height(&sizes[..at]).max(column_height(&sizes[at..]))).unwrap_or(sizes.len())
}

/// A column of sections as lines: faded uppercase headers, keys right-aligned in the accent.
fn column_lines<'a>(sections: &[Section], width: usize, theme: &Theme) -> Vec<Line<'a>> {
    let key_w = sections.iter().flat_map(|s| s.rows.iter().map(|row| row.keys.width())).max().unwrap_or(0);
    let what_w = width.saturating_sub(key_w + KEY_GAP);
    let header = Style::default().fg(theme.faded).add_modifier(Modifier::BOLD);
    let key = Style::default().fg(theme.accent).add_modifier(Modifier::BOLD);
    let mut lines = vec![];
    for (i, section) in sections.iter().enumerate() {
        if i > 0 {
            lines.push(Line::default());
        }
        lines.push(Line::from(Span::styled(section.title.to_uppercase(), header)));
        for row in &section.rows {
            lines.push(Line::from(vec![
                Span::styled(format!("{:>key_w$}", row.keys), key),
                Span::raw(" ".repeat(KEY_GAP)),
                Span::raw(truncate(row.does, what_w)),
            ]));
        }
    }
    lines
}

/// The widest line a column needs: its widest key, the gap, its widest description.
fn natural_width(sections: &[Section]) -> usize {
    let key_w = sections.iter().flat_map(|s| s.rows.iter().map(|row| row.keys.width())).max().unwrap_or(0);
    let what_w = sections.iter().flat_map(|s| s.rows.iter().map(|row| row.does.width())).max().unwrap_or(0);
    let title_w = sections.iter().map(|s| s.title.width()).max().unwrap_or(0);
    (key_w + KEY_GAP + what_w).max(title_w)
}

/// The overlay, centred, at most 90 % of the screen each way; it scrolls by `scroll` rows when taller than that.
pub fn draw(f: &mut Frame<'_>, theme: &Theme, area: Rect, help: Help, sections: &[Section]) {
    let sizes = sizes(sections);
    let max_w = area.width * 9 / 10;
    let two = max_w >= TWO_COLUMNS_FROM;
    let at = if two { split(&sizes) } else { sections.len() };
    let (left, right) = sections.split_at(at);
    let frame_x = usize::from(2 + 2 * PAD_X);
    let natural = if two { natural_width(left) + GUTTER + natural_width(right) } else { natural_width(left) };
    let width = ((natural + frame_x) as u16).min(max_w).max(area.width.min(40));
    let inner_w = usize::from(width).saturating_sub(frame_x);
    let (left_w, right_w) = if two {
        let share = inner_w.saturating_sub(GUTTER);
        let left_w = share * natural_width(left) / (natural_width(left) + natural_width(right)).max(1);
        (left_w, share - left_w)
    } else {
        (inner_w, 0)
    };
    let left_lines = column_lines(left, left_w, theme);
    let right_lines = column_lines(right, right_w, theme);
    let rows = left_lines.len().max(right_lines.len());
    let frame_y = usize::from(2 + 2 * PAD_Y);
    let height = ((rows + frame_y) as u16).min(area.height * 9 / 10).max(area.height.min(8));
    let visible = usize::from(height).saturating_sub(frame_y);
    let top = help.scroll.min(rows.saturating_sub(visible));
    let popup = Rect { x: area.x + (area.width - width) / 2, y: area.y + (area.height - height) / 2, width, height };
    let more = if help.every_key { "" } else { "? every key · " };
    let hint = if rows > visible {
        format!(" {}–{} of {} · j k scroll · {more}esc close ", top + 1, (top + visible).min(rows), rows)
    } else {
        format!(" {more}esc close ")
    };
    let block = pane::framed(theme, "keys", true)
        .title_bottom(Line::from(Span::styled(hint, Style::default().fg(theme.faded))).right_aligned())
        .padding(Padding::new(PAD_X, PAD_X, PAD_Y, PAD_Y));
    let inner = block.inner(popup);
    f.render_widget(Clear, popup);
    f.render_widget(block.style(Style::default().bg(theme.surface)), popup);
    let scroll_to = (top as u16, 0);
    let left_area = Rect { width: left_w as u16, ..inner };
    f.render_widget(Paragraph::new(left_lines).scroll(scroll_to), left_area);
    if two {
        let right_area = Rect { x: inner.x + (left_w + GUTTER) as u16, width: right_w as u16, ..inner };
        f.render_widget(Paragraph::new(right_lines).scroll(scroll_to), right_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_columns_end_as_close_as_possible_without_reordering() {
        assert_eq!(split(&[3, 3]), 1);
        assert_eq!(split(&[20, 1, 1, 1]), 1, "one tall section stays alone on the left");
        assert_eq!(split(&[1, 1, 1, 1, 10]), 4);
    }

    #[test]
    fn a_column_counts_headers_and_the_blank_rows_between_sections() {
        assert_eq!(column_height(&[2, 3]), 3 + 4 + 1);
        assert_eq!(column_height(&[]), 0);
    }

    #[test]
    fn the_last_row_is_every_section_in_one_column() {
        let row = || Row { keys: "j".to_owned(), does: "down" };
        let sections = [Section { title: "move", rows: vec![row(), row()] }, Section { title: "app", rows: vec![row()] }];
        assert_eq!(last_row(&sections), 5);
        assert_eq!(last_row(&[]), 0);
    }
}
