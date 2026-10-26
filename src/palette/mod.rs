//! The palette, `ctrl-k` or `⌘k`: one line that finds things or runs commands, picked by the first character as
//! VS Code does. The app names its modes and parses the line; the palette keeps what is typed, the highlighted
//! row, the history, and tab completion with a ghost suggestion.
use crate::complete::{self, Cycle};
use crate::fuzzy;
use crate::toward::Toward;

pub mod view;

/// How many rows the palette's list shows.
pub const MAX_SHOWN: usize = 12;

/// What the palette searches, as the app defines it. The default mode is the one an empty line steps back to.
pub trait Mode: Copy + Default + PartialEq {
    /// The mode `first`, typed on an empty line in this mode, switches to instead of being typed.
    fn switched_by(self, first: char) -> Option<Self>;

    /// Whether the line is a command: `↑ ↓` walk the commands while it names one, and tab completes it.
    fn takes_commands(self) -> bool;
}

/// A palette command: the word typed after `:`, and what the list says it does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verb {
    /// The word typed.
    pub word: &'static str,
    /// What the list says it does.
    pub does: &'static str,
}

/// The commands the list shows for what is typed: all of them on an empty line, else ranked by
/// how well their word matches the first word typed, at most [`MAX_SHOWN`].
pub fn verbs_for(input: &str, verbs: impl IntoIterator<Item = Verb>) -> Vec<Verb> {
    let typed = input.split_whitespace().next().unwrap_or("");
    let ranked: Vec<Verb> = if typed.is_empty() {
        verbs.into_iter().collect()
    } else {
        fuzzy::rank(typed, verbs.into_iter().map(|verb| (verb.word, verb))).into_iter().map(|(_, verb)| verb).collect()
    };
    ranked.into_iter().take(MAX_SHOWN).collect()
}

/// The palette while open, in mode `M`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Palette<M> {
    /// What it searches now.
    pub mode: M,
    /// What is typed after the mode's character.
    pub input: String,
    /// The highlighted row of the list.
    pub selected: usize,
    /// The lines submitted before, oldest first.
    pub history: Vec<String>,
    history_at: Option<usize>,
    cycle: Option<Cycling>,
}

/// A cycle over the last token, and the line before it that stays put.
#[derive(Clone, Debug, PartialEq)]
struct Cycling {
    prefix: String,
    cycle: Cycle,
}

impl<M: Mode> Palette<M> {
    /// Opens on `mode`, with the lines submitted before to recall.
    pub fn new(mode: M, history: Vec<String>) -> Self {
        Self { mode, input: String::new(), selected: 0, history, history_at: None, cycle: None }
    }

    /// A first character the mode switches on changes the mode instead of being typed.
    pub fn type_char(&mut self, c: char) {
        match self.mode.switched_by(c).filter(|_| self.input.is_empty()) {
            Some(mode) => self.mode = mode,
            None => self.input.push(c),
        }
        self.selected = 0;
        self.cycle = None;
    }

    /// Erases a character; on an empty line it steps back to the default mode first. `false` once there is
    /// nothing left to erase, which closes the palette.
    pub fn backspace(&mut self) -> bool {
        self.selected = 0;
        self.cycle = None;
        if self.input.pop().is_some() {
            return true;
        }
        let stepped = self.mode != M::default();
        self.mode = M::default();
        stepped
    }

    /// Still on the command's name: `↑ ↓` walk the list of commands, not the arguments.
    pub fn naming_command(&self) -> bool {
        self.mode.takes_commands() && !self.input.contains(char::is_whitespace)
    }

    /// Moves the highlight in a list of `len` rows, staying inside it.
    pub fn move_by(&mut self, delta: isize, len: usize) {
        self.selected = if len == 0 { 0 } else { self.selected.saturating_add_signed(delta).min(len - 1) };
    }

    /// Replaces the token being typed with the next candidate for its slot.
    pub fn complete(&mut self, candidates: &[String], toward: Toward) {
        let cycling = if let Some(mut cycling) = self.cycle.take() {
            cycling.cycle.advance(toward);
            cycling
        } else {
            let (prefix, token) = split_last_token(&self.input);
            let Some(cycle) = Cycle::new(token, candidates) else { return };
            Cycling { prefix: prefix.to_owned(), cycle }
        };
        let chosen = cycling.cycle.current();
        let trailing = if chosen.ends_with('=') { "" } else { " " };
        self.input = format!("{}{chosen}{trailing}", cycling.prefix);
        self.cycle = Some(cycling);
    }

    /// The grey rest of the best completion for the token being typed, as a shell shows it.
    pub fn ghost(&self, candidates: &[String]) -> Option<String> {
        if self.cycle.is_some() {
            return None;
        }
        let (_, token) = split_last_token(&self.input);
        complete::ghost(token, candidates)
    }

    /// Accepts the ghost text, as `→` does in a shell.
    pub fn accept(&mut self, candidates: &[String]) {
        if let Some(rest) = self.ghost(candidates) {
            self.input.push_str(&rest);
            if !self.input.ends_with('=') {
                self.input.push(' ');
            }
        }
    }

    /// The options tab is cycling through, the one in hand between brackets.
    pub fn hint(&self) -> Option<String> {
        Some(self.cycle.as_ref()?.cycle.hint(str::to_owned))
    }

    /// Recalls the line submitted before the one shown.
    pub fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let at = self.history_at.map_or(self.history.len() - 1, |i| i.saturating_sub(1));
        self.history_at = Some(at);
        self.input.clone_from(&self.history[at]);
        self.cycle = None;
    }

    /// Recalls the line submitted after the one shown, or an empty line past the last.
    pub fn history_down(&mut self) {
        let Some(at) = self.history_at else { return };
        if at + 1 >= self.history.len() {
            self.history_at = None;
            self.input.clear();
        } else {
            self.history_at = Some(at + 1);
            self.input.clone_from(&self.history[at + 1]);
        }
        self.cycle = None;
    }

    /// Takes the line, remembers it, and leaves the palette empty for the next command.
    pub fn submit(&mut self) -> String {
        let line = std::mem::take(&mut self.input).trim().to_owned();
        if !line.is_empty() && self.history.last() != Some(&line) {
            self.history.push(line.clone());
        }
        self.history_at = None;
        self.cycle = None;
        line
    }
}

fn split_last_token(input: &str) -> (&str, &str) {
    match input.rfind(char::is_whitespace) {
        Some(i) => (&input[..=i], &input[i + 1..]),
        None => ("", input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Things to find by default, commands after a `>`.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    enum Searching {
        #[default]
        Things,
        Commands,
    }

    impl Mode for Searching {
        fn switched_by(self, first: char) -> Option<Self> {
            (self == Self::Things && first == '>').then_some(Self::Commands)
        }

        fn takes_commands(self) -> bool {
            self == Self::Commands
        }
    }

    const VERBS: [Verb; 3] =
        [Verb { word: "go", does: "open one" }, Verb { word: "publish", does: "send it" }, Verb { word: "quit", does: "leave" }];

    fn typed(text: &str) -> Palette<Searching> {
        let mut p = Palette::new(Searching::Commands, vec![]);
        text.chars().for_each(|c| p.type_char(c));
        p
    }

    #[test]
    fn tab_cycles_and_the_ghost_suggests() {
        let words: Vec<String> = VERBS.iter().map(|verb| verb.word.to_owned()).collect();
        let mut p = typed("pu");
        assert_eq!(p.ghost(&words).as_deref(), Some("blish"));
        p.accept(&words);
        assert_eq!(p.input, "publish ");
        let things = vec!["!42".to_owned(), "!41".to_owned()];
        let mut p = typed("go 4");
        p.complete(&things, Toward::Next);
        assert_eq!(p.input, "go !42 ");
        assert!(p.hint().unwrap().starts_with("[!42]"));
        p.complete(&things, Toward::Next);
        assert_eq!(p.input, "go !41 ");
    }

    #[test]
    fn the_first_character_picks_the_mode_and_backspace_steps_back() {
        let mut p = Palette::<Searching>::default();
        p.type_char('>');
        assert_eq!((p.mode, p.input.as_str()), (Searching::Commands, ""));
        p.type_char('>');
        assert_eq!(p.input, ">", "only the first character switches");
        assert!(p.backspace() && p.backspace());
        assert_eq!(p.mode, Searching::Things, "an empty line steps back to the default mode");
        assert!(!p.backspace(), "nothing left: the palette closes");
        p.type_char('/');
        assert_eq!((p.mode, p.input.as_str()), (Searching::Things, "/"), "a slash is typed like any letter");
    }

    #[test]
    fn the_highlight_stays_inside_the_list() {
        let mut p = Palette::<Searching>::default();
        p.move_by(5, 3);
        assert_eq!(p.selected, 2);
        p.move_by(-9, 3);
        assert_eq!(p.selected, 0);
        p.move_by(1, 0);
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn the_command_list_follows_the_typed_verb() {
        assert_eq!(verbs_for("", VERBS), VERBS);
        assert_eq!(verbs_for("pub", VERBS).first().map(|verb| verb.word), Some("publish"));
        assert_eq!(verbs_for("go !42", VERBS).first().map(|verb| verb.word), Some("go"));
        assert!(typed("pu").naming_command() && !typed("go !4").naming_command());
    }

    #[test]
    fn history_recall() {
        let mut p = typed("go !1");
        assert_eq!(p.submit(), "go !1");
        p.input = "quit".into();
        p.submit();
        p.history_up();
        assert_eq!(p.input, "quit");
        p.history_up();
        assert_eq!(p.input, "go !1");
        p.history_down();
        p.history_down();
        assert_eq!(p.input, "");
    }
}
