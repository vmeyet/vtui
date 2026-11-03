//! A list narrowed as fzf does: the typed line and what it matches, ranked again only when the line or the list changes.
//! The app says what each key does and draws the rows.
use super::{Mode, Palette};
use crate::fuzzy;
use crate::toward::Toward;

/// The only mode of a plain search: what is typed is matched, never read as a command.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlainSearch;

impl Mode for PlainSearch {
    fn switched_by(self, _first: char) -> Option<Self> {
        None
    }

    fn takes_commands(self) -> bool {
        false
    }
}

/// What a search lists, matched on its text.
pub trait Searched: Clone {
    /// The text the typed line is matched against.
    fn search_text(&self) -> String;
}

/// The typed line, and what it matches, best first.
#[derive(Clone, Debug, PartialEq)]
pub struct Search<T, M = PlainSearch> {
    /// The typed line and the highlighted row.
    pub palette: Palette<M>,
    /// What matches the line, best first.
    pub shown: Vec<T>,
}

impl<T: Searched, M: Mode> Search<T, M> {
    /// Every item of `all`, in its order, before anything is typed.
    pub fn new(all: &[T]) -> Self {
        Self::with_palette(Palette::default(), all)
    }

    /// Every item of `all` that matches what `palette` already holds; a palette with lines to recall comes in this way.
    pub fn with_palette(palette: Palette<M>, all: &[T]) -> Self {
        let mut search = Self { palette, shown: vec![] };
        search.rank(all);
        search
    }

    /// Matches `all` against the line again, keeping the highlight inside the list.
    pub fn rank(&mut self, all: &[T]) {
        let ranked = fuzzy::rank(&self.palette.input, all.iter().map(|item| (item.search_text(), item)));
        self.shown = ranked.into_iter().map(|(_, item)| item.clone()).collect();
        self.palette.move_by(0, self.shown.len());
    }

    /// Types `text` on the line as [`Palette::type_str`] does, then ranks once.
    pub fn paste(&mut self, text: &str, all: &[T]) {
        self.palette.type_str(text);
        self.rank(all);
    }

    /// `↑` or `↓`, as [`Palette::arrow`] says; a line recalled narrows the list too.
    pub fn arrow(&mut self, toward: Toward, all: &[T]) {
        if self.palette.arrow(toward, self.shown.len()) {
            self.rank(all);
        }
    }

    /// The index in `shown` of the first match in a view of `rows`, scrolled so the highlighted one shows.
    pub fn first_in_view(&self, rows: usize) -> usize {
        self.palette.selected.saturating_sub(rows.saturating_sub(1))
    }

    /// The matches in a view of `rows`.
    pub fn in_view(&self, rows: usize) -> &[T] {
        let first = self.first_in_view(rows);
        &self.shown[first..self.shown.len().min(first + rows)]
    }

    /// The highlighted match.
    pub fn selected(&self) -> Option<&T> {
        self.shown.get(self.palette.selected)
    }
}

#[cfg(test)]
mod tests {
    use super::super::CommandLine;
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Table(&'static str);

    impl Searched for Table {
        fn search_text(&self) -> String {
            self.0.to_owned()
        }
    }

    const TABLES: [Table; 3] = [Table("orders"), Table("customers"), Table("order_lines")];

    fn searched(text: &str) -> Search<Table> {
        let mut search = Search::new(&TABLES);
        text.chars().for_each(|c| search.palette.type_char(c));
        search.rank(&TABLES);
        search
    }

    #[test]
    fn nothing_typed_keeps_the_order_of_the_list() {
        assert_eq!(Search::<Table>::new(&TABLES).shown, TABLES);
    }

    #[test]
    fn typing_keeps_what_matches_the_line() {
        assert_eq!(searched("cust").shown, [Table("customers")]);
    }

    #[test]
    fn a_shorter_list_keeps_the_highlight_inside_it() {
        let mut search = Search::<Table>::new(&TABLES);
        search.palette.move_by(2, 3);
        search.palette.input = "cust".into();
        search.rank(&TABLES);
        assert_eq!(search.selected(), Some(&Table("customers")));
    }

    #[test]
    fn a_paste_narrows_the_list_once_cleaned() {
        let mut search = Search::<Table>::new(&TABLES);
        search.paste("customers\n", &TABLES);
        assert_eq!(search.shown, [Table("customers")]);
    }

    #[test]
    fn nothing_matching_selects_nothing() {
        assert_eq!(searched("zzz").selected(), None);
    }

    #[test]
    fn the_view_scrolls_so_the_highlighted_one_shows() {
        let mut search = Search::<Table>::new(&TABLES);
        search.palette.move_by(2, 3);
        assert_eq!((search.first_in_view(2), search.in_view(2)), (1, &TABLES[1..3]));
    }

    #[test]
    fn a_plain_search_never_recalls_a_line() {
        let mut search = Search::with_palette(Palette::new(PlainSearch, vec!["orders".to_owned()]), &TABLES);
        search.arrow(Toward::Previous, &TABLES);
        assert_eq!((search.palette.input.as_str(), search.shown.len()), ("", 3));
    }

    #[test]
    fn a_recalled_line_narrows_the_list() {
        let mut search = Search::with_palette(Palette::new(CommandLine, vec!["cust".to_owned()]), &TABLES);
        search.arrow(Toward::Previous, &TABLES);
        assert_eq!(search.shown, [Table("customers")]);
    }

    #[test]
    fn the_arrows_move_the_highlight_while_the_line_stays() {
        let mut search = Search::<Table>::new(&TABLES);
        search.arrow(Toward::Next, &TABLES);
        assert_eq!(search.selected(), Some(&Table("customers")));
    }
}
