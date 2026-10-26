//! Text cut to a number of terminal columns, so a wide character or an emoji never crosses the edge.
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

    proptest::proptest! {
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
