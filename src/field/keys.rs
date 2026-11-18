//! The editing keys every input box shares, so a key that works in one box works in the next.
//! The app keeps the keys that mean something only to it, such as the `enter` that sends.
use super::Field;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl Field {
    /// Applies `key` as a box of several lines reads it: the keys of [`Field::press_on_one_line`], `↑` and `↓` between
    /// the lines, and a new line on the keys [`adds_a_line`] names. A key that edits nothing changes nothing.
    pub fn press(&mut self, key: KeyEvent) {
        match key.code {
            _ if adds_a_line(key) => self.insert('\n'),
            KeyCode::Up => self.up(),
            KeyCode::Down => self.down(),
            _ => self.press_on_one_line(key),
        }
    }

    /// Applies `key` as a box of one line reads it: `←` `→` by character, `⌥←` `⌥→` or `alt-b` `alt-f` by word,
    /// `home` `end` or `ctrl-a` `ctrl-e` to the ends, `backspace` and `delete`, `⌥⌫` or `ctrl-w` for a word, and a
    /// character typed at the cursor. It never adds a line, and leaves `↑`, `↓`, `enter` and `tab` to the app.
    pub fn press_on_one_line(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        match key.code {
            KeyCode::Left | KeyCode::Char('b') if alt => self.word_left(),
            KeyCode::Right | KeyCode::Char('f') if alt => self.word_right(),
            KeyCode::Backspace if alt => self.delete_word(),
            KeyCode::Left => self.left(),
            KeyCode::Right => self.right(),
            KeyCode::Home => self.start(),
            KeyCode::End => self.end(),
            KeyCode::Char('a') if ctrl => self.start(),
            KeyCode::Char('e') if ctrl => self.end(),
            KeyCode::Char('w') if ctrl => self.delete_word(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete(),
            KeyCode::Char(c) if !ctrl => self.insert(c),
            _ => {}
        }
    }
}

/// `alt-enter`, `shift-enter` and `ctrl-j` start a new line, so a box tells them from the `enter` that sends.
pub fn adds_a_line(key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Enter => key.modifiers.intersects(KeyModifiers::ALT | KeyModifiers::SHIFT),
        KeyCode::Char('j') => key.modifiers.contains(KeyModifiers::CONTROL),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    fn code(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn alt(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::ALT)
    }

    fn ctrl(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn pressed(text: &str, keys: &[KeyEvent], press: impl Fn(&mut Field, KeyEvent)) -> String {
        let mut field = Field::new(text);
        for key in keys {
            press(&mut field, *key);
        }
        field.text().to_owned()
    }

    fn text_after(text: &str, keys: &[KeyEvent]) -> String {
        pressed(text, keys, Field::press)
    }

    fn line_after(text: &str, keys: &[KeyEvent]) -> String {
        pressed(text, keys, Field::press_on_one_line)
    }

    #[test]
    fn alt_enter_shift_enter_and_ctrl_j_each_add_a_line() {
        let shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
        for adds_line in [alt(KeyCode::Enter), shift_enter, ctrl('j')] {
            assert_eq!(text_after("fix", &[adds_line, key('x')]), "fix\nx", "{adds_line:?}");
        }
    }

    #[test]
    fn a_plain_enter_edits_nothing() {
        assert_eq!(text_after("fix", &[code(KeyCode::Enter), key('x')]), "fixx");
    }

    #[test]
    fn alt_b_jumps_a_word_back_and_types_no_letter() {
        assert_eq!(text_after("fix the bug", &[alt(KeyCode::Char('b')), key('x')]), "fix the xbug");
    }

    #[test]
    fn alt_f_jumps_a_word_on_and_types_no_letter() {
        assert_eq!(text_after("fix the bug", &[code(KeyCode::Home), alt(KeyCode::Char('f')), key('x')]), "fixx the bug");
    }

    #[test]
    fn a_ctrl_letter_with_no_meaning_types_nothing() {
        assert_eq!(text_after("fix", &[ctrl('o'), key('x')]), "fixx");
    }

    #[test]
    fn up_and_down_move_between_the_lines_of_a_box_of_several() {
        assert_eq!(text_after("fix\nbug", &[code(KeyCode::Up), key('x'), code(KeyCode::Down), key('y')]), "fixx\nbugy");
    }

    #[test]
    fn a_box_of_one_line_never_adds_a_line() {
        let shift_enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::SHIFT);
        for adds_line in [alt(KeyCode::Enter), shift_enter, ctrl('j')] {
            assert_eq!(line_after("fix", &[adds_line, key('x')]), "fixx", "{adds_line:?}");
        }
    }

    #[test]
    fn a_box_of_one_line_leaves_up_and_down_to_the_app() {
        assert_eq!(line_after("fix\nbug", &[code(KeyCode::Up), key('x')]), "fix\nbugx");
    }

    #[test]
    fn alt_backspace_and_ctrl_w_delete_the_word_before_the_cursor() {
        for deletes_word in [alt(KeyCode::Backspace), ctrl('w')] {
            assert_eq!(line_after("fix the bug", &[alt(KeyCode::Left), deletes_word]), "fix bug", "{deletes_word:?}");
        }
    }

    #[test]
    fn ctrl_a_and_ctrl_e_jump_to_the_ends() {
        assert_eq!(line_after("fix", &[ctrl('a'), key('x'), ctrl('e'), key('y')]), "xfixy");
    }

    #[test]
    fn delete_erases_the_character_under_the_cursor() {
        assert_eq!(line_after("fix", &[code(KeyCode::Home), code(KeyCode::Delete)]), "ix");
    }
}
