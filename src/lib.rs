//! Terminal UI building blocks that know nothing of the app using them: the terminal itself, its background,
//! a theme, an input box, fuzzy matching and completion, the `ctrl-k` palette, the `?` overlay,
//! the clipboard, the user's editor for long text, and opt-in usage counts.
pub mod clipboard;
pub mod complete;
pub mod editor;
pub mod field;
pub mod fuzzy;
pub mod ground;
pub mod help;
mod log;
pub mod motion;
pub mod palette;
pub mod pane;
pub mod screen;
pub mod text;
pub mod theme;
pub mod toward;
pub mod usage;
