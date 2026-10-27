//! Completing the token being typed in any input box, the palette's or an app's own:
//! a cycle through the candidates, and a ghost of the best one.
//!
//! The candidates the token starts, case aside, come first: shortest first, then in the order given.
//! Fuzzy matches fill the room left, best score first, then in the order given.
//! An empty token offers the first candidates as given.
use crate::fuzzy;
use crate::toward::Toward;
use std::cmp::Reverse;
use std::ops::Range;

/// How many candidates tab-cycling walks through.
const MAX: usize = 50;

/// The candidates of a long list, built once: each name with its lowercase key, sorted by it,
/// so the names a token starts are found by binary search instead of a scan of the list.
#[derive(Clone, Debug, Default)]
pub struct Index {
    names: Vec<Name>,
    /// Where the first candidates as given sit in `names`, in that order.
    head: Vec<usize>,
}

#[derive(Clone, Debug)]
struct Name {
    key: String,
    text: String,
    /// Its place in the candidates as given, which breaks every tie.
    at: usize,
}

impl Index {
    /// Lowercases and sorts `candidates` once; a changed list is a new index.
    pub fn new(candidates: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut names: Vec<Name> = candidates.into_iter().enumerate().map(|(at, text)| Name::new(at, text.into())).collect();
        names.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        let mut head = vec![0; names.len().min(MAX)];
        names.iter().enumerate().filter(|(_, name)| name.at < MAX).for_each(|(i, name)| head[name.at] = i);
        Self { names, head }
    }

    /// The grey text a shell shows after the cursor: the rest of the shortest candidate `token` starts,
    /// or nothing when the token is empty or starts none longer than itself.
    /// A fuzzy match is never shown, so typing a name to its end never jumps somewhere else.
    pub fn ghost(&self, token: &str) -> Option<String> {
        if token.is_empty() {
            return None;
        }
        let key = token.to_lowercase();
        let longer = self.names[self.started_by(&key)].iter().filter(|name| name.key.len() > key.len());
        let best = longer.min_by_key(|name| name.rank())?;
        Some(after_typed(&best.text, token.chars().count()))
    }

    /// Starts a cycle on the best of at most 50 candidates for `token`, or nothing when none matches.
    pub fn cycle(&self, token: &str) -> Option<Cycle> {
        let options: Vec<String> = self.ranked(token).into_iter().map(|name| name.text.clone()).collect();
        (!options.is_empty()).then_some(Cycle { options, at: 0 })
    }

    fn ranked(&self, token: &str) -> Vec<&Name> {
        if token.is_empty() {
            return self.head.iter().map(|&i| &self.names[i]).collect();
        }
        let key = token.to_lowercase();
        let started = self.started_by(&key);
        let mut ranked = first_by_key(self.names[started.clone()].iter().collect(), MAX, |name| name.rank());
        if ranked.len() < MAX {
            ranked.extend(self.fuzzy_matches(token, started, MAX - ranked.len()));
        }
        ranked
    }

    /// Where the names `key` starts sit: one binary search for the first, one for the end of the run.
    fn started_by(&self, key: &str) -> Range<usize> {
        let from = self.names.partition_point(|name| name.key.as_str() < key);
        let count = self.names[from..].partition_point(|name| name.key.starts_with(key));
        from..from + count
    }

    /// The best fuzzy matches among the names outside `started`, at most `room` of them.
    fn fuzzy_matches(&self, token: &str, started: Range<usize>, room: usize) -> Vec<&Name> {
        let others = self.names[..started.start].iter().chain(&self.names[started.end..]);
        let scored = fuzzy::rank(token, others.map(|name| (&name.text, name)));
        let best = first_by_key(scored, room, |(score, name)| (Reverse(*score), name.at));
        best.into_iter().map(|(_, name)| name).collect()
    }
}

impl Name {
    fn new(at: usize, text: String) -> Self {
        Self { key: text.to_lowercase(), text, at }
    }

    /// Its order among the names a token starts: shorter first, then as given.
    fn rank(&self) -> (usize, usize) {
        (self.key.len(), self.at)
    }
}

/// The first `count` of `items` by `key`, in order, without sorting the rest.
fn first_by_key<T, K: Ord>(mut items: Vec<T>, count: usize, key: impl Fn(&T) -> K) -> Vec<T> {
    if items.len() > count {
        items.select_nth_unstable_by_key(count - 1, &key);
        items.truncate(count);
    }
    items.sort_unstable_by_key(key);
    items
}

/// What follows the first `typed` characters of `name`.
fn after_typed(name: &str, typed: usize) -> String {
    // Lowercasing can change byte lengths (İ is 2 bytes, its lowercase 3), so the typed part is skipped by characters.
    let rest = name.char_indices().nth(typed).map_or(name.len(), |(i, _)| i);
    name[rest..].to_owned()
}

/// [`Index::ghost`] for a short list, the index built on each call; a long list keeps its [`Index`].
pub fn ghost<S: AsRef<str>>(token: &str, candidates: &[S]) -> Option<String> {
    Index::new(candidates.iter().map(AsRef::as_ref)).ghost(token)
}

/// The candidates one token is being cycled through, until the text changes again.
#[derive(Clone, Debug, PartialEq)]
pub struct Cycle {
    options: Vec<String>,
    at: usize,
}

impl Cycle {
    /// [`Index::cycle`] for a short list, the index built on each call; a long list keeps its [`Index`].
    pub fn new<S: AsRef<str>>(token: &str, candidates: &[S]) -> Option<Self> {
        Index::new(candidates.iter().map(AsRef::as_ref)).cycle(token)
    }

    /// Moves to the next or previous candidate, wrapping around at either end.
    pub fn advance(&mut self, toward: Toward) {
        self.at = toward.wrapped(self.at, 1, self.options.len());
    }

    /// The candidate in hand, the one the token is replaced with.
    pub fn current(&self) -> &str {
        &self.options[self.at]
    }

    /// The row shown while cycling: the option in hand between brackets, then the next few.
    /// `label` is how each one reads on screen, which needs not be the value itself.
    pub fn hint(&self, label: impl Fn(&str) -> String) -> String {
        let shown = self.options.iter().enumerate().skip(self.at).take(4);
        shown.map(|(i, o)| if i == self.at { format!("[{}]", label(o)) } else { label(o) }).collect::<Vec<_>>().join("  ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<String> {
        ["rocket", "rock", "racing_car", "tada"].map(String::from).to_vec()
    }

    fn options(token: &str, names: &[&str]) -> Vec<String> {
        Index::new(names.iter().copied()).cycle(token).map_or_else(Vec::new, |cycle| cycle.options)
    }

    fn numbered(prefix: &str, count: usize) -> Vec<String> {
        (0..count).map(|n| format!("{prefix}{n:02}")).collect()
    }

    #[test]
    fn the_names_a_token_starts_come_shortest_first_then_as_given() {
        assert_eq!(options("ro", &["rocket", "robe", "rock", "rod"]), ["rod", "robe", "rock", "rocket"]);
    }

    #[test]
    fn the_names_a_token_starts_come_before_fuzzy_matches() {
        assert_eq!(options("rc", &["racing_car", "arc", "rc_car"]), ["rc_car", "racing_car", "arc"]);
    }

    #[test]
    fn fuzzy_matches_tied_on_score_keep_the_order_given() {
        assert_eq!(options("4", &["!42", "!41"]), ["!42", "!41"]);
    }

    #[test]
    fn a_full_prefix_tier_leaves_no_room_for_fuzzy_matches() {
        let mut names = numbered("a", 60);
        names.insert(0, "ba".to_owned());
        let options = Index::new(names).cycle("a").expect("matches").options;
        assert_eq!(options.len(), MAX);
        assert!(options.iter().all(|o| o.starts_with('a')));
    }

    #[test]
    fn fuzzy_matches_fill_the_room_the_prefix_tier_leaves_up_to_the_cap() {
        let mut names = numbered("xa", 60);
        names.push("a".to_owned());
        let options = Index::new(names).cycle("a").expect("matches").options;
        assert_eq!(options.len(), MAX);
        assert_eq!(options[..2], ["a", "xa00"]);
    }

    #[test]
    fn an_empty_token_offers_the_first_candidates_as_given() {
        assert_eq!(options("", &["rocket", "rock", "racing_car", "tada"]), candidates());
        let names = numbered("n", 60);
        assert_eq!(Index::new(names.clone()).cycle("").expect("names").options, names[..MAX]);
    }

    #[test]
    fn nothing_matching_starts_no_cycle() {
        assert_eq!(Index::new(candidates()).cycle("zzz"), None);
        assert_eq!(Index::default().cycle(""), None);
    }

    #[test]
    fn the_ghost_completes_the_shortest_name_the_token_starts() {
        let index = Index::new(["rockets", "rocket", "rock"]);
        assert_eq!(index.ghost("rock").as_deref(), Some("et"), "a name no longer than the token has nothing to show");
        assert_eq!(index.ghost("ROCK").as_deref(), Some("et"), "case does not matter");
    }

    #[test]
    fn the_ghost_shows_nothing_for_an_empty_token_or_a_fuzzy_match() {
        assert_eq!(ghost("", &candidates()), None);
        assert_eq!(ghost("zzz", &candidates()), None);
        assert_eq!(ghost("rcar", &candidates()), None);
    }

    #[test]
    fn the_ghost_survives_letters_whose_lowercase_changes_length() {
        let people = vec!["@İlker".to_owned()];
        assert_eq!(ghost("@i", &people).as_deref(), Some("lker"));
        assert_eq!(ghost("@İ", &people).as_deref(), Some("lker"));
    }

    #[test]
    fn the_small_list_helpers_answer_like_the_index() {
        let index = Index::new(candidates());
        assert_eq!(ghost("ro", &candidates()), index.ghost("ro"));
        assert_eq!(Cycle::new("ro", &candidates()), index.cycle("ro"));
    }

    #[test]
    fn borrowed_names_complete_like_owned_ones() {
        let names = ["rocket", "rock", "racing_car", "tada"];
        assert_eq!(ghost("rock", &names), ghost("rock", &candidates()));
        assert_eq!(Cycle::new("rock", &names), Cycle::new("rock", &candidates()));
    }

    #[test]
    fn cycling_walks_the_candidates_both_ways_and_wraps() {
        let mut cycle = Cycle::new("rock", &candidates()).expect("matches");
        assert_eq!(cycle.current(), "rock");
        cycle.advance(Toward::Next);
        assert_eq!(cycle.current(), "rocket");
        cycle.advance(Toward::Next);
        assert_eq!(cycle.current(), "rock", "two matches, so it wraps");
        cycle.advance(Toward::Previous);
        assert_eq!(cycle.current(), "rocket");
    }

    #[test]
    fn the_hint_marks_the_option_in_hand_and_labels_them_all() {
        let cycle = Cycle::new("rock", &candidates()).expect("matches");
        assert_eq!(cycle.hint(str::to_owned), "[rock]  rocket");
        assert_eq!(cycle.hint(|o| format!("· {o}")), "[· rock]  · rocket");
        let mark = "›";
        assert_eq!(cycle.hint(|o| format!("{mark}{o}")), "[›rock]  ›rocket", "a label can borrow what is around it");
    }
}
