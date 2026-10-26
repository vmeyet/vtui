//! Terminal UI building blocks that know nothing of the app using them: the terminal itself, its background,
//! a theme, an input box, fuzzy matching, the `ctrl-k` palette, the `?` overlay and the clipboard.
pub mod clipboard;
mod complete;
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
