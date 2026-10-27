//! Text and styled lines cut or wrapped to a number of terminal columns, so a wide character or an emoji never crosses the edge.
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// `text` cut to `width` columns, ending with `…` when it did not fit.
pub fn truncate(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    with_ellipsis(text.chars(), width)
}

/// `text` cut to `width` columns from its middle, so both of its ends still read: `src/pay/…/charge.rs`.
pub fn truncate_middle(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    let room = width.saturating_sub(1);
    let head = fitting(text.chars(), room / 2);
    let tail: String = fitting(text.chars().rev(), room - room / 2).chars().rev().collect();
    format!("{head}…{tail}")
}

/// `text` with each tab drawn as `tab`, cut as `truncate` cuts, without drawing the tabs of a long line past the cut.
pub fn truncate_tabs(text: &str, tab: &str, width: usize) -> String {
    let drawn = text.split('\t').map(UnicodeWidthStr::width).sum::<usize>() + text.matches('\t').count() * tab.width();
    if drawn <= width {
        return text.replace('\t', tab);
    }
    let chars = text.split('\t').enumerate().flat_map(|(i, piece)| if i == 0 { "" } else { tab }.chars().chain(piece.chars()));
    with_ellipsis(chars, width)
}

/// `line` cut to `width` columns from its end, each span keeping its style, the cut one ending with `…`.
pub fn truncate_line(line: &Line<'_>, width: usize) -> Line<'static> {
    let spans = if line.width() <= width {
        line.spans.iter().map(|span| Span::styled(span.content.to_string(), span.style)).collect()
    } else {
        cut_spans(&line.spans, width)
    };
    with_spans(line, spans)
}

/// `line` wrapped to `width` columns, so an app can count the rows it takes and draw those same rows.
///
/// It breaks between characters (grapheme clusters), not words, so an accent or an emoji sequence stays whole.
/// A character wider than `width` takes a row alone, and a `width` of 0 wraps like 1.
/// Each span keeps its style, pieces of one style on a row are merged, and each row keeps the line's own style.
pub fn wrap_line(line: &Line<'_>, width: usize) -> Vec<Line<'static>> {
    let width = width.max(1);
    let characters = line.spans.iter().flat_map(|span| span.content.graphemes(true).map(|character| (character, span.style)));
    let mut rows = Vec::new();
    let mut row: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    for (character, style) in characters {
        let taken = character.width();
        if used + taken > width && used > 0 {
            rows.push(std::mem::take(&mut row));
            used = 0;
        }
        used += taken;
        match row.last_mut() {
            Some(last) if last.style == style => last.content.to_mut().push_str(character),
            _ => row.push(Span::styled(character.to_owned(), style)),
        }
    }
    rows.push(row);
    rows.into_iter().map(|spans| with_spans(line, spans)).collect()
}

/// `spans` laid out as `line` is: its own style and alignment.
fn with_spans(line: &Line<'_>, spans: Vec<Span<'static>>) -> Line<'static> {
    Line { spans, style: line.style, alignment: line.alignment }
}

/// The first `spans` that fit in `width - 1` columns, the one that crosses the cut ending with `…`.
fn cut_spans(spans: &[Span<'_>], width: usize) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let mut room = width - 1;
    let mut kept = Vec::new();
    for span in spans {
        let text = fitting(span.content.chars(), room);
        if text.width() < span.content.width() {
            kept.push(Span::styled(format!("{text}…"), span.style));
            break;
        }
        room -= text.width();
        kept.push(Span::styled(text, span.style));
    }
    kept
}

/// The first `chars` that fit in `width - 1` columns, then `…`.
fn with_ellipsis(chars: impl Iterator<Item = char>, width: usize) -> String {
    format!("{}…", fitting(chars, width.saturating_sub(1)))
}

/// The first `chars` that fit in `room` columns.
fn fitting(chars: impl Iterator<Item = char>, room: usize) -> String {
    let mut used = 0;
    chars
        .take_while(|&c| {
            used += UnicodeWidthChar::width(c).unwrap_or(0);
            used <= room
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::strategy::Strategy;
    use ratatui::style::{Color, Style};
    use ratatui::text::Span;

    fn badges() -> Line<'static> {
        Line::from(vec![
            Span::styled("nina", Style::default().fg(Color::Green)),
            Span::raw(" · "),
            Span::styled("acme/widgets", Style::default().fg(Color::Red)),
        ])
    }

    fn spans(line: &Line<'_>) -> Vec<(String, Style)> {
        line.spans.iter().map(|span| (span.content.to_string(), span.style)).collect()
    }

    #[test]
    fn truncate_keeps_the_width_and_ends_with_an_ellipsis() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("a rather long title", 8), "a rathe…");
        assert_eq!(truncate("héllo wörld", 6).width(), 6);
        assert_eq!(truncate("漢漢漢", 4), "漢…", "a wide character that would cross the cut stays out");
    }

    #[test]
    fn a_cut_from_the_middle_keeps_both_ends() {
        assert_eq!(truncate_middle("src/a.rs", 20), "src/a.rs");
        assert_eq!(truncate_middle("src/tui/app/compact/diff_view.rs", 20), "src/tui/a…ff_view.rs");
        assert_eq!(truncate_middle("漢漢漢漢漢", 6), "漢…漢", "a wide character that would cross the cut stays out");
        assert_eq!(truncate_middle("abcdef", 1), "…");
    }

    #[test]
    fn a_line_that_fits_is_kept_whole() {
        assert_eq!(truncate_line(&badges(), 40), badges());
    }

    #[test]
    fn a_cut_line_keeps_the_style_of_each_span_and_ends_with_an_ellipsis() {
        let green = Style::default().fg(Color::Green);
        let red = Style::default().fg(Color::Red);
        assert_eq!(
            spans(&truncate_line(&badges(), 10)),
            [("nina".to_owned(), green), (" · ".to_owned(), Style::default()), ("ac…".to_owned(), red)]
        );
    }

    #[test]
    fn a_line_cut_to_no_column_is_empty() {
        assert_eq!(truncate_line(&badges(), 0).width(), 0);
    }

    #[test]
    fn a_cut_line_keeps_its_own_style() {
        let line = Line::styled("nina reviewed acme/widgets", Style::default().fg(Color::Blue));
        assert_eq!(truncate_line(&line, 6).style, line.style);
    }

    fn rows(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_line_that_fits_wraps_to_itself() {
        assert_eq!(wrap_line(&badges(), 40), [badges()]);
    }

    #[test]
    fn a_wrapped_line_keeps_the_style_of_each_span() {
        let green = Style::default().fg(Color::Green);
        let red = Style::default().fg(Color::Red);
        let wrapped = wrap_line(&badges(), 6);
        assert_eq!(rows(&wrapped), ["nina ·", " acme/", "widget", "s"]);
        assert_eq!(spans(&wrapped[0]), [("nina".to_owned(), green), (" ·".to_owned(), Style::default())]);
        assert_eq!(spans(&wrapped[1]), [(" ".to_owned(), Style::default()), ("acme/".to_owned(), red)]);
    }

    #[test]
    fn spans_of_one_style_on_one_row_are_merged() {
        let line = Line::from(vec![Span::raw("ni"), Span::raw("na"), Span::raw(" reviewed")]);
        assert_eq!(spans(&wrap_line(&line, 6)[0]), [("nina r".to_owned(), Style::default())]);
    }

    #[test]
    fn a_wide_character_that_would_cross_the_edge_starts_the_next_row() {
        assert_eq!(rows(&wrap_line(&Line::raw("a漢漢"), 4)), ["a漢", "漢"]);
    }

    #[test]
    fn a_letter_keeps_its_combining_accent() {
        assert_eq!(rows(&wrap_line(&Line::raw("nine\u{301}"), 3)), ["nin", "e\u{301}"]);
    }

    #[test]
    fn an_emoji_sequence_or_a_flag_is_never_split() {
        assert_eq!(rows(&wrap_line(&Line::raw("a👩\u{200d}💻🇫🇷"), 2)), ["a", "👩\u{200d}💻", "🇫🇷"]);
    }

    #[test]
    fn a_character_wider_than_the_width_takes_a_row_alone() {
        assert_eq!(rows(&wrap_line(&Line::raw("a漢b"), 1)), ["a", "漢", "b"]);
    }

    #[test]
    fn a_width_of_zero_wraps_like_a_width_of_one() {
        assert_eq!(wrap_line(&badges(), 0), wrap_line(&badges(), 1));
    }

    #[test]
    fn an_empty_line_wraps_to_one_empty_row() {
        assert_eq!(rows(&wrap_line(&Line::default(), 10)), [""]);
    }

    #[test]
    fn each_wrapped_row_keeps_the_style_of_the_line() {
        let line = Line::styled("nina reviewed acme/widgets", Style::default().fg(Color::Blue));
        assert!(wrap_line(&line, 6).iter().all(|row| row.style == line.style));
    }

    fn lines_of_clusters() -> impl Strategy<Value = Line<'static>> {
        proptest::collection::vec("(a|é|e\u{301}|漢|🦀|👩\u{200d}💻|🇫🇷| ){0,8}", 0..4)
            .prop_map(|words| Line::from(words.into_iter().map(Span::raw).collect::<Vec<_>>()))
    }

    proptest::proptest! {
        #[test]
        fn a_wrapped_row_never_takes_more_than_its_width_unless_one_character_does(line in lines_of_clusters(), width in 1..8usize) {
            for row in wrap_line(&line, width) {
                proptest::prop_assert!(row.width() <= width || row.to_string().graphemes(true).count() == 1);
            }
        }

        #[test]
        fn wrapped_rows_read_back_as_the_line(line in lines_of_clusters(), width in 0..8usize) {
            proptest::prop_assert_eq!(rows(&wrap_line(&line, width)).concat(), line.to_string());
        }

        #[test]
        fn a_cut_line_never_takes_more_than_its_width(words in proptest::collection::vec("[a-zé漢🦀 ]{0,8}", 0..4), width in 0..24usize) {
            let line = Line::from(words.into_iter().map(Span::raw).collect::<Vec<_>>());
            proptest::prop_assert!(truncate_line(&line, width).width() <= width);
        }

        #[test]
        fn a_cut_text_never_takes_more_than_its_width(text in "[a-zé漢🦀 ]{0,20}", width in 1..24usize) {
            proptest::prop_assert!(truncate(&text, width).width() <= width);
        }

        #[test]
        fn a_text_cut_from_the_middle_never_takes_more_than_its_width(text in "[a-zé漢🦀/ ]{0,20}", width in 1..24usize) {
            proptest::prop_assert!(truncate_middle(&text, width).width() <= width);
        }

        #[test]
        fn tabs_drawn_while_cutting_read_as_tabs_drawn_first(text in "[a-z漢\t ]{0,20}", width in 1..24usize) {
            proptest::prop_assert_eq!(truncate_tabs(&text, "→   ", width), truncate(&text.replace('\t', "→   "), width));
        }
    }
}
