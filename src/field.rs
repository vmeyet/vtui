//! The input box: its text and where the cursor sits, in one value so the two cannot drift.
//! The cursor is a byte offset kept on a character boundary, so an accent or an emoji never splits.

use std::fmt;

/// A line of text being typed, or several, with the cursor in it.
#[derive(Clone, Default)]
pub struct Field {
    text: String,
    cursor: usize,
    revision: u64,
    secret: bool,
}

/// The revision is history and the secret mark is how the field prints, not content: neither takes part.
impl PartialEq for Field {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text && self.cursor == other.cursor
    }
}

/// A secret field prints how many characters it holds, never what they are.
impl fmt::Debug for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Field");
        if self.secret {
            debug.field("text", &format_args!("<redacted, {} chars>", self.text.chars().count()));
        } else {
            debug.field("text", &self.text);
        }
        debug.field("cursor", &self.cursor).field("revision", &self.revision).finish()
    }
}

impl Field {
    /// Starts on `text` with the cursor after its last character, where typing goes on.
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        Self { cursor: text.len(), text, revision: 0, secret: false }
    }

    /// Starts like [`Field::new`], but its `Debug` never prints the text, so personal data kept in it
    /// stays out of logs. Only `Debug` hides it: [`Field::text`] still hands it over.
    pub fn secret(text: impl Into<String>) -> Self {
        Self { secret: true, ..Self::new(text) }
    }

    /// Starts on `before` then `after`, with the cursor between them: where a completed word ends.
    pub fn joined(before: &str, after: &str) -> Self {
        Self { cursor: before.len(), text: format!("{before}{after}"), revision: 0, secret: false }
    }

    /// What is typed.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Hands the text over and leaves the row empty.
    pub fn take(&mut self) -> String {
        if !self.text.is_empty() {
            self.changed();
        }
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }

    fn at_end(&self) -> bool {
        self.cursor == self.text.len()
    }

    /// What sits before the cursor, the character under it (empty past the last one), and the rest.
    pub fn split(&self) -> (&str, &str, &str) {
        let under = self.next().map_or("", |end| &self.text[self.cursor..end]);
        (&self.text[..self.cursor], under, &self.text[self.cursor + under.len()..])
    }

    /// What sits before the word under the cursor, the word, and the rest; whitespace ends a word,
    /// so the word is empty when the cursor sits between two spaces.
    pub fn word(&self) -> (&str, &str, &str) {
        let start = self.text[..self.cursor].char_indices().rfind(|(_, c)| c.is_whitespace()).map_or(0, |(i, c)| i + c.len_utf8());
        let end = self.text[self.cursor..].find(char::is_whitespace).map_or(self.text.len(), |i| self.cursor + i);
        (&self.text[..start], &self.text[start..end], &self.text[end..])
    }

    /// A count that grows with every change to the text and only then, so an app can tell the text moved on
    /// since it last looked without keeping a copy. Two fields with the same text and cursor are equal whatever it is.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Types `c` at the cursor.
    pub fn insert(&mut self, c: char) {
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.changed();
    }

    /// Types `text` at the cursor as one edit, the cursor after it.
    pub fn insert_str(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.changed();
    }

    /// Types what was pasted at the cursor as one edit: line endings become `\n`, a tab four spaces, and control
    /// characters that could move the cursor or reorder the text on screen are dropped.
    pub fn paste(&mut self, text: &str) {
        self.insert_str(&clean_paste(text));
    }

    /// Erases the character before the cursor; nothing at the start.
    pub fn backspace(&mut self) {
        let Some(prev) = self.prev() else { return };
        self.text.remove(prev);
        self.cursor = prev;
        self.changed();
    }

    /// Erases the character under the cursor; nothing at the end.
    pub fn delete(&mut self) {
        if !self.at_end() {
            self.text.remove(self.cursor);
            self.changed();
        }
    }

    /// Deletes back to the start of the word the cursor follows, the spaces in between included.
    pub fn delete_word(&mut self) {
        let from = self.word_start();
        if from == self.cursor {
            return;
        }
        self.text.replace_range(from..self.cursor, "");
        self.cursor = from;
        self.changed();
    }

    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// One character back; nothing at the start.
    pub fn left(&mut self) {
        self.cursor = self.prev().unwrap_or(0);
    }

    /// One character on; nothing at the end.
    pub fn right(&mut self) {
        self.cursor = self.next().unwrap_or(self.cursor);
    }

    /// To the start of the word the cursor follows, as `⌥←` does in a macOS text field.
    pub fn word_left(&mut self) {
        self.cursor = self.word_start();
    }

    /// To the end of the word ahead of the cursor, as `⌥→` does in a macOS text field.
    pub fn word_right(&mut self) {
        let rest = &self.text[self.cursor..];
        let word = rest.trim_start();
        let skipped = rest.len() - word.len();
        self.cursor += skipped + word.find(char::is_whitespace).unwrap_or(word.len());
    }

    /// To the start of the text.
    pub fn start(&mut self) {
        self.cursor = 0;
    }

    /// To the end of the text.
    pub fn end(&mut self) {
        self.cursor = self.text.len();
    }

    /// To the line above, at the same column or the end of a shorter line; nothing on the first line.
    pub fn up(&mut self) {
        let start = self.line_start(self.cursor);
        if start > 0 {
            self.cursor = self.at_column(self.line_start(start - 1), self.column());
        }
    }

    /// To the line below, at the same column or the end of a shorter line; nothing on the last line.
    pub fn down(&mut self) {
        let end = self.line_end(self.cursor);
        if end < self.text.len() {
            self.cursor = self.at_column(end + 1, self.column());
        }
    }

    fn line_start(&self, at: usize) -> usize {
        self.text[..at].rfind('\n').map_or(0, |newline| newline + 1)
    }

    fn line_end(&self, at: usize) -> usize {
        self.text[at..].find('\n').map_or(self.text.len(), |newline| at + newline)
    }

    /// How many characters sit between the start of the cursor's line and the cursor.
    fn column(&self) -> usize {
        self.text[self.line_start(self.cursor)..self.cursor].chars().count()
    }

    /// The offset `column` characters into the line starting at `start`, or that line's end.
    fn at_column(&self, start: usize, column: usize) -> usize {
        let end = self.line_end(start);
        self.text[start..end].char_indices().nth(column).map_or(end, |(offset, _)| start + offset)
    }

    fn prev(&self) -> Option<usize> {
        self.text[..self.cursor].char_indices().next_back().map(|(i, _)| i)
    }

    fn next(&self) -> Option<usize> {
        self.text[self.cursor..].chars().next().map(|c| self.cursor + c.len_utf8())
    }

    fn word_start(&self) -> usize {
        let word = self.text[..self.cursor].trim_end();
        match word.char_indices().rev().find(|(_, c)| c.is_whitespace()) {
            Some((i, space)) => i + space.len_utf8(),
            None => 0,
        }
    }
}

/// Pasted text made safe to type: `\r\n` and a lone `\r` become `\n`, a tab four spaces, and other control
/// characters and bidi controls are dropped, as they could move the cursor or reorder the text on screen.
pub(crate) fn clean_paste(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                chars.next_if_eq(&'\n');
                cleaned.push('\n');
            }
            '\t' => cleaned.push_str("    "),
            '\n' => cleaned.push('\n'),
            c if c.is_control() || is_bidi_control(c) => {}
            c => cleaned.push(c),
        }
    }
    cleaned
}

fn is_bidi_control(c: char) -> bool {
    matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The text with the cursor drawn in it, so a test reads as what the row shows.
    fn shown(field: &Field) -> String {
        let (before, under, after) = field.split();
        format!("{before}|{under}{after}")
    }

    fn typed(text: &str) -> Field {
        let mut field = Field::default();
        for c in text.chars() {
            field.insert(c);
        }
        field
    }

    #[test]
    fn typing_leaves_the_cursor_after_the_last_character() {
        assert_eq!(shown(&typed("hey")), "hey|");
        assert_eq!(typed("hey").text(), "hey");
    }

    #[test]
    fn prefilled_text_starts_with_the_cursor_at_the_end() {
        assert_eq!(shown(&Field::new("hey")), "hey|");
        assert_eq!(shown(&Field::default()), "|");
    }

    #[test]
    fn insert_lands_where_the_cursor_is() {
        let mut field = Field::new("hey");
        field.left();
        field.insert('l');
        assert_eq!(shown(&field), "hel|y");
    }

    #[test]
    fn movement_stops_at_both_ends() {
        let mut field = Field::new("ab");
        field.right();
        assert_eq!(shown(&field), "ab|");
        field.left();
        field.left();
        field.left();
        assert_eq!(shown(&field), "|ab");
    }

    #[test]
    fn word_jumps_land_on_word_edges_and_stop_at_both_ends() {
        let mut field = typed("fix  the\nbug");
        field.word_left();
        assert_eq!(shown(&field), "fix  the\n|bug");
        field.word_left();
        assert_eq!(shown(&field), "fix  |the\nbug");
        field.word_left();
        field.word_left();
        assert_eq!(shown(&field), "|fix  the\nbug");
        field.word_right();
        assert_eq!(shown(&field), "fix|  the\nbug");
        field.word_right();
        field.word_right();
        assert_eq!(shown(&field), "fix  the\nbug|");
        field.word_right();
        assert_eq!(shown(&field), "fix  the\nbug|");
    }

    #[test]
    fn word_jumps_step_over_accents_and_emoji_whole() {
        let mut field = typed("été 🎉ok");
        field.word_left();
        assert_eq!(shown(&field), "été |🎉ok");
        field.start();
        field.word_right();
        assert_eq!(shown(&field), "été| 🎉ok");
    }

    #[test]
    fn home_and_end_jump_to_the_edges() {
        let mut field = Field::new("hey");
        field.start();
        assert_eq!(shown(&field), "|hey");
        field.end();
        assert_eq!(shown(&field), "hey|");
    }

    #[test]
    fn up_and_down_keep_the_column_and_stop_at_the_first_and_last_line() {
        let mut field = typed("first line\nab\nthird line");
        field.up();
        assert_eq!(shown(&field), "first line\nab|\nthird line", "a shorter line takes the cursor to its end");
        field.up();
        assert_eq!(shown(&field), "fi|rst line\nab\nthird line", "the column comes from the line left");
        field.up();
        assert_eq!(shown(&field), "fi|rst line\nab\nthird line");
        field.down();
        field.down();
        assert_eq!(shown(&field), "first line\nab\nth|ird line");
        field.down();
        assert_eq!(shown(&field), "first line\nab\nth|ird line");
    }

    #[test]
    fn up_and_down_count_the_column_in_characters() {
        let mut field = typed("été\nabcd");
        field.left();
        field.up();
        assert_eq!(shown(&field), "été|\nabcd");
        field.left();
        field.down();
        assert_eq!(shown(&field), "été\nab|cd");
    }

    #[test]
    fn backspace_eats_the_character_before_the_cursor_and_nothing_at_the_start() {
        let mut field = Field::new("hey");
        field.backspace();
        assert_eq!(shown(&field), "he|");
        field.start();
        field.backspace();
        assert_eq!(shown(&field), "|he");
        field.right();
        field.backspace();
        assert_eq!(shown(&field), "|e");
    }

    #[test]
    fn delete_eats_the_character_under_the_cursor_and_nothing_at_the_end() {
        let mut field = Field::new("hey");
        field.delete();
        assert_eq!(shown(&field), "hey|");
        field.start();
        field.delete();
        assert_eq!(shown(&field), "|ey");
        field.right();
        field.delete();
        assert_eq!(shown(&field), "e|");
    }

    #[test]
    fn delete_word_takes_the_word_before_the_cursor_with_its_trailing_spaces() {
        let mut field = Field::new("ship it now");
        field.delete_word();
        assert_eq!(shown(&field), "ship it |");

        let mut spaced = Field::new("ship it   ");
        spaced.delete_word();
        assert_eq!(shown(&spaced), "ship |");

        let mut only_word = Field::new("ship");
        only_word.delete_word();
        assert_eq!(shown(&only_word), "|");

        let mut empty = Field::default();
        empty.delete_word();
        assert_eq!(shown(&empty), "|");
    }

    #[test]
    fn delete_word_keeps_what_follows_the_cursor() {
        let mut field = Field::new("ship it now");
        field.start();
        field.right();
        field.right();
        field.right();
        field.right();
        field.delete_word();
        assert_eq!(shown(&field), "| it now");
    }

    #[test]
    fn accents_move_and_delete_as_one_character() {
        let mut field = Field::new("café");
        field.left();
        assert_eq!(shown(&field), "caf|é");
        field.insert('è');
        assert_eq!(shown(&field), "cafè|é");
        field.backspace();
        assert_eq!(shown(&field), "caf|é");
        field.delete();
        assert_eq!(shown(&field), "caf|");
    }

    #[test]
    fn emoji_move_and_delete_as_one_character() {
        let mut field = Field::new("a🚀b");
        field.start();
        field.right();
        assert_eq!(shown(&field), "a|🚀b");
        field.right();
        assert_eq!(shown(&field), "a🚀|b");
        field.backspace();
        assert_eq!(shown(&field), "a|b");
        field.insert('🎉');
        assert_eq!(shown(&field), "a🎉|b");
        field.delete();
        assert_eq!(shown(&field), "a🎉|");
    }

    #[test]
    fn delete_word_splits_multi_byte_words_on_their_spaces() {
        let mut field = Field::new("café 🚀 fusée");
        field.delete_word();
        assert_eq!(shown(&field), "café 🚀 |");
        field.delete_word();
        assert_eq!(shown(&field), "café |");
    }

    #[test]
    fn take_hands_the_text_over_and_resets_the_row() {
        let mut field = Field::new("café");
        assert_eq!(field.take(), "café");
        assert_eq!(shown(&field), "|");
        field.insert('a');
        assert_eq!(shown(&field), "a|");
        field.take();
        assert_eq!(shown(&field), "|");
    }

    #[test]
    fn the_word_spans_the_cursor_up_to_the_spaces_around_it() {
        let mut field = Field::new("hi @nina there");
        assert_eq!(field.word(), ("hi @nina ", "there", ""));
        for _ in 0..8 {
            field.left();
        }
        assert_eq!(shown(&field), "hi @ni|na there");
        assert_eq!(field.word(), ("hi ", "@nina", " there"));
        field.start();
        assert_eq!(field.word(), ("", "hi", " @nina there"));
        assert_eq!(Field::new("café ").word(), ("café ", "", ""));
        assert_eq!(Field::new("a\u{3000}🚀").word(), ("a\u{3000}", "🚀", ""), "a wide space ends a word too");
    }

    #[test]
    fn joined_puts_the_cursor_between_the_two_sides() {
        assert_eq!(shown(&Field::joined("hi :tada: ", " there")), "hi :tada: | there");
        assert_eq!(shown(&Field::joined("café", "")), "café|");
    }

    #[test]
    fn insert_str_lands_at_the_cursor_and_leaves_the_cursor_after_it() {
        let mut field = Field::new("hi there");
        field.word_left();
        field.insert_str("@nina ");
        assert_eq!(shown(&field), "hi @nina |there");
    }

    #[test]
    fn paste_types_the_cleaned_text_at_the_cursor() {
        let mut field = Field::new("ab");
        field.left();
        field.paste("x\r\ny\tz");
        assert_eq!(shown(&field), "ax\ny    z|b");
    }

    #[test]
    fn a_cleaned_paste_turns_every_line_ending_into_a_newline() {
        assert_eq!(clean_paste("one\r\ntwo\rthree\nfour"), "one\ntwo\nthree\nfour");
    }

    #[test]
    fn a_cleaned_paste_turns_a_tab_into_four_spaces() {
        assert_eq!(clean_paste("a\tb"), "a    b");
    }

    #[test]
    fn a_cleaned_paste_drops_control_characters() {
        assert_eq!(clean_paste("a\u{0}b\u{1b}[31mc\u{7f}d\u{85}e\u{9f}f"), "ab[31mcdef");
    }

    #[test]
    fn a_cleaned_paste_drops_bidi_controls() {
        assert_eq!(clean_paste("\u{202a}a\u{202e}b\u{2066}c\u{2069}"), "abc");
    }

    #[test]
    fn a_cleaned_paste_keeps_accents_and_emoji() {
        assert_eq!(clean_paste("café 🚀"), "café 🚀");
    }

    #[test]
    fn every_edit_that_changes_the_text_grows_the_revision() {
        let mut field = Field::new("acme widgets");
        let mut seen = vec![field.revision()];
        let edits: [fn(&mut Field); 7] = [
            |field| field.insert('!'),
            |field| field.insert_str("ok"),
            |field| field.paste("go"),
            Field::backspace,
            |field| {
                field.start();
                field.delete();
            },
            |field| {
                field.end();
                field.delete_word();
            },
            |field| {
                field.take();
            },
        ];
        for edit in edits {
            edit(&mut field);
            seen.push(field.revision());
        }
        assert!(seen.is_sorted() && seen.windows(2).all(|pair| pair[0] != pair[1]), "{seen:?}");
    }

    #[test]
    fn moving_the_cursor_keeps_the_revision() {
        let mut field = Field::new("acme widgets\nnina");
        let before = field.revision();
        field.left();
        field.word_left();
        field.up();
        field.down();
        field.right();
        field.word_right();
        field.start();
        field.end();
        assert_eq!(field.revision(), before);
    }

    #[test]
    fn an_edit_that_changes_nothing_keeps_the_revision() {
        let mut field = Field::default();
        let before = field.revision();
        field.backspace();
        field.delete();
        field.delete_word();
        field.take();
        field.insert_str("");
        field.paste("\u{0}");
        assert_eq!(field.revision(), before);
    }

    #[test]
    fn fields_with_the_same_text_and_cursor_are_equal_whatever_their_history() {
        let mut edited = Field::new("nina");
        edited.insert('!');
        edited.backspace();
        assert_eq!(edited, Field::new("nina"));
    }

    #[test]
    fn a_secret_field_prints_its_length_instead_of_its_text() {
        let printed = format!("{:?}", Field::secret("nina@acme"));
        assert!(!printed.contains("nina"), "{printed}");
        assert!(printed.contains("<redacted, 9 chars>"), "{printed}");
    }

    #[test]
    fn a_secret_field_stays_secret_through_edits_take_and_clone() {
        let mut field = Field::secret("café");
        field.insert('!');
        field.paste("nina");
        field.backspace();
        field.delete_word();
        field.insert_str("acme");
        field.take();
        field.insert_str("widgets");
        let printed = format!("{:?}", field.clone());
        assert!(!printed.contains("widgets"), "{printed}");
        assert!(printed.contains("<redacted, 7 chars>"), "{printed}");
    }

    #[test]
    fn a_plain_field_prints_its_text() {
        assert!(format!("{:?}", Field::new("nina")).contains("\"nina\""));
    }
}
