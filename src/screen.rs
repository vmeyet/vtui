//! The terminal as the app holds it: raw mode on the alternate screen, the mouse wheel and drags, focus changes, and ⌘
//! reported where the terminal speaks the kitty keyboard protocol (Ghostty, Kitty, `WezTerm`,
//! iTerm2 with the option on), so ⌘K reaches the palette like ctrl-k does.
use crate::log::OrWarn as _;
use crossterm::event::{
    DisableFocusChange, EnableFocusChange, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{EnterAlternateScreen, enable_raw_mode};
use ratatui::DefaultTerminal;
use std::io::Write;

/// Clicks, drags and the wheel, in the SGR encoding. Not crossterm's `EnableMouseCapture`: it also
/// asks for every pointer move without a button down, which would wake the loop for nothing.
const MOUSE_ON: &[u8] = b"\x1b[?1000h\x1b[?1002h\x1b[?1006h";
const MOUSE_OFF: &[u8] = b"\x1b[?1006l\x1b[?1002l\x1b[?1000l";

/// What the app asked of the terminal, so it can take it back exactly.
#[derive(Debug)]
pub struct Screen {
    taken: Taken,
}

/// Whether the app reads the mouse, or leaves it to the terminal so its own text selection keeps working.
#[derive(Clone, Copy, Debug)]
enum Mouse {
    Captured,
    Left,
}

impl Mouse {
    fn on(self) -> &'static [u8] {
        match self {
            Self::Captured => MOUSE_ON,
            Self::Left => b"",
        }
    }

    fn off(self) -> &'static [u8] {
        match self {
            Self::Captured => MOUSE_OFF,
            Self::Left => b"",
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
    /// Focus changes tell the app when the terminal goes to the background and comes back.
    fn take_mouse_and_focus(self) {
        write(self.mouse.on());
        crossterm::execute!(std::io::stdout(), EnableFocusChange).or_warn("asking for focus changes");
    }

    fn give_back(self) {
        write(self.mouse.off());
        crossterm::execute!(std::io::stdout(), DisableFocusChange).or_warn("stopping focus changes");
        if self.modifiers {
            stop_modifiers();
        }
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
        Self::take(Mouse::Left)
    }

    fn take(mouse: Mouse) -> (DefaultTerminal, Self) {
        let terminal = ratatui::init();
        let modifiers = crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false) && report_modifiers();
        let screen = Self { taken: Taken { mouse, modifiers } };
        screen.take_extras_back_on_panic();
        screen.taken.take_mouse_and_focus();
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
        self.taken.take_mouse_and_focus();
        terminal.clear().or_warn("clearing the terminal");
        outcome
    }
}

fn write(sequence: &[u8]) {
    let mut out = std::io::stdout();
    out.write_all(sequence).and_then(|()| out.flush()).or_warn("writing to the terminal");
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

    #[test]
    fn a_captured_mouse_is_asked_for_and_given_back() {
        assert_eq!(Mouse::Captured.on(), MOUSE_ON);
        assert_eq!(Mouse::Captured.off(), MOUSE_OFF);
    }

    #[test]
    fn a_mouse_left_to_the_terminal_sends_nothing_either_way() {
        assert_eq!(Mouse::Left.on(), b"");
        assert_eq!(Mouse::Left.off(), b"");
    }
}
