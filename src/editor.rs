//! Writing long text in the user's `$VISUAL` or `$EDITOR`: the draft goes to a private temp file, what comes back is kept.
//! The editor needs a plain terminal, so an app calls [`edit`] inside [`crate::screen::Screen::lend`].
use std::io;
use std::path::Path;
use std::process::{Command, ExitStatus};

const FALLBACK: &str = "vi";

/// What the editor gave back.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edited {
    /// The file came back untouched or empty: the user backed out.
    Cancelled,
    /// The text the user wrote, without the blank lines around it.
    Done(String),
    /// The text changed but the editor exited with an error, as vim does on `:cq` or after any error in the session.
    Errored(String),
}

/// Opens `draft` in the user's editor and gives back what came of it. The temp file is named
/// `{prefix}<random>.md`, so the editor highlights Markdown and the user sees what the text is for; it is gone either way.
/// A program that cannot be run is an error naming it.
pub fn edit(draft: &str, prefix: &str) -> io::Result<Edited> {
    let file = tempfile::Builder::new().prefix(prefix).suffix(".md").tempfile()?;
    std::fs::write(file.path(), draft)?;
    let status = Editor::from_env().open(file.path())?;
    let edited = std::fs::read_to_string(file.path())?;
    Ok(to_send(draft, &edited, status.success()))
}

fn to_send(draft: &str, edited: &str, editor_ok: bool) -> Edited {
    let text = edited.trim();
    let is_cancelled = text.is_empty() || text == draft.trim();
    match (is_cancelled, editor_ok) {
        (true, _) => Edited::Cancelled,
        (false, true) => Edited::Done(text.to_owned()),
        (false, false) => Edited::Errored(text.to_owned()),
    }
}

/// The editor to open, and the arguments it was configured with, as in `EDITOR="code -w"`.
#[derive(Debug, PartialEq, Eq)]
struct Editor {
    program: String,
    args: Vec<String>,
}

impl Editor {
    fn from_env() -> Self {
        Self::pick(var("VISUAL"), var("EDITOR"))
    }

    fn pick(visual: Option<String>, editor: Option<String>) -> Self {
        let command = visual.or(editor).unwrap_or_default();
        let mut words = command.split_whitespace();
        let program = words.next().unwrap_or(FALLBACK).to_owned();
        Self { program, args: words.map(str::to_owned).collect() }
    }

    fn open(&self, path: &Path) -> io::Result<ExitStatus> {
        Command::new(&self.program)
            .args(&self.args)
            .arg(path)
            .status()
            .map_err(|e| io::Error::new(e.kind(), format!("could not run {}: {e}", self.program)))
    }
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(program: &str, args: &[&str]) -> Editor {
        Editor { program: program.into(), args: args.iter().map(|a| (*a).to_owned()).collect() }
    }

    #[test]
    fn visual_comes_first_then_editor_then_vi() {
        assert_eq!(Editor::pick(Some("emacs".into()), Some("nano".into())), editor("emacs", &[]));
        assert_eq!(Editor::pick(None, Some("nano".into())), editor("nano", &[]));
        assert_eq!(Editor::pick(None, None), editor("vi", &[]));
        assert_eq!(Editor::pick(Some("  ".into()), None), editor("vi", &[]));
    }

    #[test]
    fn an_editor_keeps_the_arguments_it_was_given() {
        assert_eq!(Editor::pick(None, Some("code -w --new-window".into())), editor("code", &["-w", "--new-window"]));
    }

    #[test]
    fn a_program_that_cannot_run_is_an_error_naming_it() {
        let error = editor("acme-widgets-no-such-editor", &[]).open(Path::new("draft.md")).expect_err("no such program");
        assert!(error.to_string().starts_with("could not run acme-widgets-no-such-editor: "), "{error}");
    }

    #[test]
    fn a_changed_file_is_sent_without_its_surrounding_blank() {
        assert_eq!(to_send("", "hello\n", true), Edited::Done("hello".into()));
        assert_eq!(to_send("hi", "\nhi there\n\n", true), Edited::Done("hi there".into()));
        assert_eq!(to_send("a", "one\ntwo", true), Edited::Done("one\ntwo".into()));
    }

    #[test]
    fn a_changed_file_is_kept_when_the_editor_exits_with_an_error() {
        assert_eq!(to_send("", "hello\n", false), Edited::Errored("hello".into()));
    }

    #[test]
    fn an_empty_or_untouched_file_cancels_whatever_the_editor_exit() {
        for editor_ok in [true, false] {
            assert_eq!(to_send("", "  \n ", editor_ok), Edited::Cancelled);
            assert_eq!(to_send("hi", "", editor_ok), Edited::Cancelled);
            assert_eq!(to_send("hi", "hi\n", editor_ok), Edited::Cancelled);
        }
    }
}
