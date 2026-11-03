//! A typed command line read into its parts, the start of every app's parser: the command's word, what follows it,
//! and which token the end of the line is on. The app keeps its own commands and what each argument means.
use crate::fuzzy;

/// How many close words an unknown command's message names.
const CLOSE_WORDS: usize = 3;

/// The token at the end of the line, the one tab completes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Typing {
    /// The command's word.
    Verb,
    /// The argument at this index, counted from 0.
    Argument(usize),
}

/// A line cut into the command's word and what follows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypedLine<'a> {
    /// The word typed, without a leading `:`; empty on a blank line.
    pub verb: &'a str,
    /// What follows the word, with the blank space around it dropped.
    pub rest: &'a str,
    /// The token at the end of the line.
    pub typing: Typing,
}

/// The two sides of `key=value`, as `:set` takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting<'a> {
    /// What is left of the first `=`.
    pub key: &'a str,
    /// What is right of it.
    pub value: &'a str,
}

impl<'a> TypedLine<'a> {
    /// Reads `line`; `:go !42` and `go !42` read the same.
    pub fn read(line: &'a str) -> Self {
        let line = line.trim_start().trim_start_matches(':').trim_start();
        let Some((verb, rest)) = line.split_once(char::is_whitespace) else { return Self { verb: line, rest: "", typing: Typing::Verb } };
        let done = rest.split_whitespace().count();
        let argument = if line.ends_with(char::is_whitespace) { done } else { done.saturating_sub(1) };
        Self { verb, rest: rest.trim(), typing: Typing::Argument(argument) }
    }

    /// What follows the word as `key=value`, when it holds a `=`.
    pub fn setting(&self) -> Option<Setting<'a>> {
        let (key, value) = self.rest.split_once('=')?;
        Some(Setting { key: key.trim(), value: value.trim() })
    }
}

/// What to say of a word no command has, naming the commands it comes close to.
pub fn unknown_verb<'a>(typed: &str, words: impl IntoIterator<Item = &'a str>) -> String {
    let close = fuzzy::suggestions(typed, words, CLOSE_WORDS);
    if close.is_empty() {
        format!("unknown command :{typed}")
    } else {
        format!("unknown command :{typed}, did you mean :{}?", close.join(", :"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORDS: [&str; 2] = ["publish", "quit"];

    #[test]
    fn a_leading_colon_and_blank_space_are_not_part_of_the_word() {
        let line = TypedLine::read(" : go  !42 ");
        assert_eq!((line.verb, line.rest), ("go", "!42"));
    }

    #[test]
    fn a_line_with_no_space_is_still_on_the_word() {
        assert_eq!(TypedLine::read("pu").typing, Typing::Verb);
        assert_eq!(TypedLine::read("").verb, "");
    }

    #[test]
    fn a_trailing_space_starts_the_next_argument() {
        assert_eq!(TypedLine::read("go ").typing, Typing::Argument(0));
        assert_eq!(TypedLine::read("go !4").typing, Typing::Argument(0));
        assert_eq!(TypedLine::read("go !42 ").typing, Typing::Argument(1));
        assert_eq!(TypedLine::read("go !42 more").typing, Typing::Argument(1));
    }

    #[test]
    fn a_setting_is_cut_at_its_first_equals_sign() {
        assert_eq!(TypedLine::read("set a = b=c").setting(), Some(Setting { key: "a", value: "b=c" }));
    }

    #[test]
    fn no_equals_sign_is_no_setting() {
        assert_eq!(TypedLine::read("set theme").setting(), None);
    }

    #[test]
    fn an_unknown_word_names_the_commands_it_comes_close_to() {
        assert_eq!(unknown_verb("pubish", WORDS), "unknown command :pubish, did you mean :publish?");
    }

    #[test]
    fn an_unknown_word_far_from_every_command_names_none() {
        assert_eq!(unknown_verb("zzzzzz", WORDS), "unknown command :zzzzzz");
    }
}
