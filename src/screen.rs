//! The terminal as the app holds it: raw mode on the alternate screen, the mouse wheel and drags, focus changes,
//! pastes in one piece, and ⌘ reported where the terminal speaks the kitty keyboard protocol (Ghostty, Kitty, `WezTerm`,
//! iTerm2 with the option on), so ⌘K reaches the palette like ctrl-k does.
use crate::log::OrWarn as _;
use crossterm::Command;
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, EnableBracketedPaste, EnableFocusChange, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::backend::Backend;
use ratatui::{DefaultTerminal, Terminal};
use std::io::Write;

/// Clicks, drags and the wheel, in the SGR encoding. Not crossterm's `EnableMouseCapture`: it also
/// asks for every pointer move without a button down, which would wake the loop for nothing.
const MOUSE_ON: &str = "\x1b[?1000h\x1b[?1002h\x1b[?1006h";
const MOUSE_OFF: &str = "\x1b[?1006l\x1b[?1002l\x1b[?1000l";

/// What the app asked of the terminal, so it can take it back exactly.
///
/// Pasted text arrives as one `Event::Paste(String)`, which the app must handle: a paste is not typed key by key,
/// so a pasted newline no longer arrives as Enter, and an app that ignores the event ignores the paste.
#[derive(Debug)]
pub struct Screen {
    taken: Taken,
}

/// Whether the app captures the mouse, or leaves it free for the terminal so its own text selection keeps working.
#[derive(Clone, Copy, Debug)]
enum Mouse {
    Captured,
    Free,
}

impl Mouse {
    fn on(self) -> &'static str {
        match self {
            Self::Captured => MOUSE_ON,
            Self::Free => "",
        }
    }

    fn off(self) -> &'static str {
        match self {
            Self::Captured => MOUSE_OFF,
            Self::Free => "",
        }
    }
}

/// What the app took on top of ratatui, copied into the panic hook so a panic gives back the same.
#[derive(Clone, Copy, Debug)]
struct Taken {
    mouse: Mouse,
    modifiers: bool,
}

impl Taken {
    fn take(self) {
        write(&self.asked());
    }

    fn give_back(self) {
        write(&self.given_back());
        if self.modifiers {
            stop_modifiers();
        }
    }

    /// The mouse, focus changes, which tell the app when the terminal goes to the background and comes back,
    /// and bracketed paste, which hands a paste over whole so its newlines are not read as Enter.
    fn asked(self) -> String {
        let mut sequence = self.mouse.on().to_owned();
        EnableFocusChange.write_ansi(&mut sequence).or_warn("asking for focus changes");
        EnableBracketedPaste.write_ansi(&mut sequence).or_warn("asking for pastes as one event");
        sequence
    }

    fn given_back(self) -> String {
        let mut sequence = self.mouse.off().to_owned();
        DisableFocusChange.write_ansi(&mut sequence).or_warn("stopping focus changes");
        DisableBracketedPaste.write_ansi(&mut sequence).or_warn("stopping pastes as one event");
        sequence
    }
}

impl Screen {
    /// Takes the terminal with the mouse. `ratatui::init` installs the panic hook that gives it back; the mouse,
    /// focus changes and the modifier report are undone first on a panic too, or the shell would read escape codes.
    pub fn enter() -> (DefaultTerminal, Self) {
        Self::take(Mouse::Captured)
    }

    /// Takes the terminal like [`Screen::enter`] but leaves the mouse to it, for an app that reads no clicks:
    /// the terminal's own text selection keeps working.
    pub fn enter_without_mouse() -> (DefaultTerminal, Self) {
        Self::take(Mouse::Free)
    }

    fn take(mouse: Mouse) -> (DefaultTerminal, Self) {
        let terminal = ratatui::init();
        let modifiers = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false) && report_modifiers();
        let screen = Self { taken: Taken { mouse, modifiers } };
        screen.take_extras_back_on_panic();
        screen.taken.take();
        (terminal, screen)
    }

    fn take_extras_back_on_panic(&self) {
        let taken = self.taken;
        let restore = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            taken.give_back();
            restore(info);
        }));
    }

    /// Gives the terminal back to the shell for good.
    pub fn leave(self) {
        self.taken.give_back();
        ratatui::restore();
    }

    /// Lends the terminal to another program while `run` lasts: the program gets a plain terminal,
    /// then the app takes it back and repaints. No new panic hook stacks up on the way.
    pub fn lend<T>(&self, terminal: &mut DefaultTerminal, run: impl FnOnce() -> T) -> T {
        self.taken.give_back();
        ratatui::restore();
        let outcome = run();
        enable_raw_mode().or_warn("turning raw mode back on");
        crossterm::execute!(std::io::stdout(), EnterAlternateScreen).or_warn("entering the alternate screen again");
        if self.taken.modifiers {
            report_modifiers();
        }
        self.taken.take();
        repaint_everything_on_next_draw(terminal).or_warn("resetting the screen for a full repaint");
        outcome
    }
}

/// Clears the screen and forgets the last frame, so the next draw paints every cell, at the terminal's current size.
/// Not `Terminal::clear`: it first reads the cursor position on stdin, where the app's key reader can swallow the answer,
/// and then fails before forgetting the frame, so the next draw paints only a diff on a blank screen.
fn repaint_everything_on_next_draw<B: Backend>(terminal: &mut Terminal<B>) -> Result<(), B::Error> {
    let size = terminal.size()?;
    terminal.resize(size.into())
}

fn write(sequence: &str) {
    let mut out = std::io::stdout();
    out.write_all(sequence.as_bytes()).and_then(|()| out.flush()).or_warn("writing to the terminal");
}

fn report_modifiers() -> bool {
    let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES;
    crossterm::execute!(std::io::stdout(), PushKeyboardEnhancementFlags(flags)).is_ok()
}

fn stop_modifiers() {
    crossterm::execute!(std::io::stdout(), PopKeyboardEnhancementFlags).or_warn("stopping the modifier report");
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Paragraph;

    fn draw_nina(terminal: &mut Terminal<TestBackend>) {
        terminal.draw(|frame| frame.render_widget(Paragraph::new("nina"), frame.area())).expect("drawing to the test backend");
    }

    #[test]
    fn a_captured_mouse_is_asked_for_and_given_back() {
        assert_eq!(Mouse::Captured.on(), MOUSE_ON);
        assert_eq!(Mouse::Captured.off(), MOUSE_OFF);
    }

    #[test]
    fn a_free_mouse_sends_nothing_either_way() {
        assert_eq!(Mouse::Free.on(), "");
        assert_eq!(Mouse::Free.off(), "");
    }

    #[test]
    fn taking_the_terminal_asks_for_pastes_as_one_event() {
        let taken = Taken { mouse: Mouse::Free, modifiers: false };
        assert!(taken.asked().contains("\x1b[?2004h"));
    }

    #[test]
    fn giving_the_terminal_back_stops_pastes_as_one_event() {
        let taken = Taken { mouse: Mouse::Free, modifiers: false };
        assert!(taken.given_back().contains("\x1b[?2004l"));
    }

    #[test]
    fn a_frame_drawn_after_the_reset_repaints_a_screen_wiped_behind_ratatui() {
        let mut terminal = Terminal::new(TestBackend::new(6, 1)).expect("a test terminal");
        draw_nina(&mut terminal);
        terminal.backend_mut().clear().expect("wiping the test screen");

        repaint_everything_on_next_draw(&mut terminal).expect("resetting the test terminal");
        draw_nina(&mut terminal);

        terminal.backend().assert_buffer_lines(["nina  "]);
    }
}
