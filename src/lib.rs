//! Terminal UI building blocks that know nothing of the app using them: the terminal itself, its background,
//! a theme, an input box, fuzzy matching, the `ctrl-k` palette, the `?` overlay and the clipboard.
pub mod clipboard;
pub mod field;
pub mod ground;
mod log;
pub mod motion;
pub mod pane;
pub mod screen;
pub mod text;
pub mod theme;
