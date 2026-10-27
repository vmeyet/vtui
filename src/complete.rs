//! Completing the token being typed in any input box, the palette's or an app's own:
//! a cycle through the candidates, and a ghost of the best one.
use crate::fuzzy;
use crate::toward::Toward;

/// How many candidates tab-cycling walks through.
const MAX: usize = 50;

/// The candidates for `token`, best first; an empty token offers the head of the list.
fn rank<S: AsRef<str>>(token: &str, candidates: &[S]) -> Vec<String> {
    let names = candidates.iter().map(AsRef::as_ref);
    if token.is_empty() {
        return names.take(MAX).map(str::to_owned).collect();
    }
    fuzzy::rank(token, names.map(|c| (c, c))).into_iter().map(|(_, c)| c.to_owned()).take(MAX).collect()
}

/// The grey text a shell shows after the cursor: the rest of the best candidate for `token`,
/// or nothing when the token is empty or matches nothing. A candidate `token` starts is preferred
/// over a fuzzy match, so typing a name to its end never jumps somewhere else.
pub fn ghost<S: AsRef<str>>(token: &str, candidates: &[S]) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    let names: Vec<&str> = candidates.iter().map(AsRef::as_ref).collect();
    let started = started_by(token, &names);
    let typed = token.chars().count();
    let by_prefix = names.iter().zip(&started).position(|(c, starts)| *starts && c.chars().count() > typed);
    let best = by_prefix.or_else(|| fuzzy::best(token, names.iter().enumerate().map(|(i, c)| (*c, i)))).filter(|&i| started[i])?;
    Some(after_typed(names[best], typed))
}

/// Whether `token` starts each name, case aside.
fn started_by(token: &str, names: &[&str]) -> Vec<bool> {
    let lower = token.to_lowercase();
    names.iter().map(|c| c.to_lowercase().starts_with(&lower)).collect()
}

/// What follows the first `typed` characters of `name`.
fn after_typed(name: &str, typed: usize) -> String {
    // Lowercasing can change byte lengths (İ is 2 bytes, its lowercase 3), so the typed part is skipped by characters.
    let rest = name.char_indices().nth(typed).map_or(name.len(), |(i, _)| i);
    name[rest..].to_owned()
}

/// The candidates one token is being cycled through, until the text changes again.
#[derive(Clone, Debug, PartialEq)]
pub struct Cycle {
    options: Vec<String>,
    at: usize,
}

impl Cycle {
    /// Starts on the best candidate for `token`, or nothing when none matches.
    pub fn new<S: AsRef<str>>(token: &str, candidates: &[S]) -> Option<Self> {
        let options = rank(token, candidates);
        (!options.is_empty()).then_some(Self { options, at: 0 })
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

    #[test]
    fn ranking_puts_the_closest_match_first_and_keeps_the_rest() {
        assert_eq!(rank("rock", &candidates())[0], "rock");
        assert!(rank("rock", &candidates()).contains(&"rocket".to_owned()));
        assert_eq!(rank("zzz", &candidates()), Vec::<String>::new());
    }

    #[test]
    fn an_empty_token_offers_every_candidate_in_order() {
        assert_eq!(rank("", &candidates()), candidates());
    }

    #[test]
    fn the_ghost_completes_what_the_token_starts() {
        assert_eq!(ghost("rock", &candidates()).as_deref(), Some("et"), "a candidate it starts wins over the fuzzy match");
        assert_eq!(ghost("ROCK", &candidates()).as_deref(), Some("et"), "case does not matter");
        assert_eq!(ghost("", &candidates()), None);
        assert_eq!(ghost("zzz", &candidates()), None);
        assert_eq!(ghost("rcar", &candidates()), None, "a fuzzy match the token does not start suggests nothing");
    }

    #[test]
    fn the_ghost_survives_letters_whose_lowercase_changes_length() {
        let people = vec!["@İlker".to_owned()];
        assert_eq!(ghost("@i", &people).as_deref(), Some("lker"));
        assert_eq!(ghost("@İ", &people).as_deref(), Some("lker"));
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
        assert_eq!(Cycle::new("zzz", &candidates()), None);
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
