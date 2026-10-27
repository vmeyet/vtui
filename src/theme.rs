//! One palette for the whole TUI, so a terminal never mixes theme colors with hardcoded ones.
//! Themes only set foregrounds and row surfaces; the terminal keeps painting its own background,
//! so pick the theme that matches the terminal's.
use ratatui::style::Color;

/// The colours every pane draws with; the app adds its own on top when it needs more.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// The name `named` finds it by.
    pub name: &'static str,
    /// The terminal background this palette was designed for; text drawn on a colored fill uses it.
    pub base: Color,
    /// Inline code and code blocks: the ground nudged a few percent toward the text.
    pub surface: Color,
    /// Fill of the selected row. None by default: the `▎` bar alone marks it, which stays
    /// invisible-proof on terminals whose background the theme cannot see.
    pub highlight: Option<Color>,
    /// Frames of the panes that do not have the keys: close to the ground, so the content leads.
    pub border: Color,
    /// Frames of the pane that has the keys, of a modal and of the compose box: one step up from `border`,
    /// the title's accent tells focus apart.
    pub border_focus: Color,
    /// Tertiary text: section headers, muted channels, receded panes.
    pub faded: Color,
    /// Secondary text: times, hints, counts.
    pub muted: Color,
    /// Titles, cursor bar, badges, your own reactions.
    pub accent: Color,
    /// Links.
    pub link: Color,
    /// Inline code.
    pub code: Color,
    /// A mention of someone, or of the reader.
    pub mention: Color,
    /// Done, passed, added.
    pub success: Color,
    /// Waiting, needs a look.
    pub warn: Color,
    /// Failed, removed, incidents.
    pub danger: Color,
    /// Author names, picked by a hash of the name.
    pub users: [Color; 6],
    /// Code colours, from the palette's own upstream scheme.
    pub syntax: Syntax,
}

/// What a piece of code is, as far as colour goes; [`Syntax`] gives every token a colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Token {
    /// A word of the language itself: `fn`, `if`, `return`.
    Keyword,
    /// A string or character literal.
    String,
    /// A comment, doc comments included.
    Comment,
    /// A number literal.
    Number,
    /// The name of a type.
    Type,
    /// The name of a function or method, where it is defined or called.
    Function,
    /// A constant, a boolean or a built-in value such as `None`.
    Constant,
    /// Brackets, delimiters and operators.
    Punctuation,
}

/// One colour per [`Token`]: what a keyword, a string or a comment looks like in this palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Syntax {
    /// [`Token::Keyword`].
    pub keyword: Color,
    /// [`Token::String`].
    pub string: Color,
    /// [`Token::Comment`].
    pub comment: Color,
    /// [`Token::Number`].
    pub number: Color,
    /// [`Token::Type`]; named `kind` because `type` is a Rust keyword.
    pub kind: Color,
    /// [`Token::Function`].
    pub function: Color,
    /// [`Token::Constant`].
    pub constant: Color,
    /// [`Token::Punctuation`], in the palette's muted tone.
    pub punctuation: Color,
}

impl Syntax {
    /// The colour `token` is drawn in.
    pub fn colour(&self, token: Token) -> Color {
        match token {
            Token::Keyword => self.keyword,
            Token::String => self.string,
            Token::Comment => self.comment,
            Token::Number => self.number,
            Token::Type => self.kind,
            Token::Function => self.function,
            Token::Constant => self.constant,
            Token::Punctuation => self.punctuation,
        }
    }
}

/// A palette's code colours from hex values, in [`Syntax`] field order: keyword, string, comment,
/// number, type, function, constant, punctuation.
const fn code([keyword, string, comment, number, kind, function, constant, punctuation]: [u32; 8]) -> Syntax {
    Syntax {
        keyword: rgb(keyword),
        string: rgb(string),
        comment: rgb(comment),
        number: rgb(number),
        kind: rgb(kind),
        function: rgb(function),
        constant: rgb(constant),
        punctuation: rgb(punctuation),
    }
}

/// The colour `0xRRGGBB`.
pub const fn rgb(hex: u32) -> Color {
    Color::Rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// `pct` percent of the way from `from` to `to`, per channel. Terminals have no alpha, so a
/// "translucent" surface is the background nudged a few percent toward the text.
pub const fn mix(from: u32, to: u32, pct: u32) -> Color {
    const fn channel(from: u32, to: u32, pct: u32, shift: u32) -> u8 {
        let (a, b) = ((from >> shift) & 0xff, (to >> shift) & 0xff);
        ((a * (100 - pct) + b * pct) / 100) as u8
    }
    Color::Rgb(channel(from, to, pct, 16), channel(from, to, pct, 8), channel(from, to, pct, 0))
}

const SURFACE_PCT: u32 = 3;
/// How far an unfocused frame sits from the ground toward the palette's own border colour.
const BORDER_PCT: u32 = 55;

impl Default for Theme {
    fn default() -> Self {
        DEFAULT
    }
}

impl Theme {
    /// Every palette, by the name `named` finds it by.
    pub const NAMES: [&'static str; 9] =
        ["default", "dracula", "catppuccin", "catppuccin-latte", "rosepine", "rosepine-dawn", "nord", "tokyonight", "monokai"];

    /// Case, spaces and dashes do not matter: `Tokyo Night`, `tokyo-night` and `tokyonight` are one theme.
    pub fn named(name: &str) -> Option<Theme> {
        let key: String = name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect();
        match key.as_str() {
            "default" | "slack" => Some(DEFAULT),
            "dracula" => Some(DRACULA),
            "catppuccin" | "catppuccinmocha" | "mocha" => Some(CATPPUCCIN),
            "catppuccinlatte" | "latte" => Some(CATPPUCCIN_LATTE),
            "rosepine" => Some(ROSEPINE),
            "rosepinedawn" | "dawn" => Some(ROSEPINE_DAWN),
            "nord" => Some(NORD),
            "tokyonight" => Some(TOKYONIGHT),
            "monokai" => Some(MONOKAI),
            _ => None,
        }
    }

    /// Whether the palette carries its own RGB ground; the default one follows the terminal's ANSI colours instead.
    pub fn knows_ground(&self) -> bool {
        matches!(self.base, Color::Rgb(..))
    }

    /// A palette that does not know its ground takes the terminal's, `0xRRGGBB`, so colours mixed over it
    /// match what is on screen. A palette with an RGB ground keeps its own.
    pub fn with_ground(self, ground: u32) -> Theme {
        if self.knows_ground() {
            return self;
        }
        Theme { base: rgb(ground), ..self }
    }

    /// The colour of the person `name`, the same on every screen and run.
    pub fn user(&self, name: &str) -> Color {
        let idx = name.trim().bytes().fold(7usize, |h, b| h.wrapping_mul(33).wrapping_add(b as usize)) % self.users.len();
        self.users[idx]
    }
}

/// The terminal's own ANSI palette, so it follows whatever the terminal already looks like.
const DEFAULT: Theme = Theme {
    name: "default",
    base: Color::Black,
    surface: Color::Indexed(234),
    highlight: None,
    border: Color::Indexed(236),
    border_focus: Color::Indexed(238),
    faded: Color::Indexed(240),
    muted: Color::Indexed(245),
    accent: Color::Cyan,
    link: Color::Blue,
    code: Color::Yellow,
    mention: Color::Magenta,
    success: Color::Green,
    warn: Color::Yellow,
    danger: Color::Red,
    users: [Color::Cyan, Color::Green, Color::Yellow, Color::Magenta, Color::Blue, Color::LightRed],
    syntax: Syntax {
        keyword: Color::Magenta,
        string: Color::Green,
        comment: Color::Indexed(244),
        number: Color::Yellow,
        kind: Color::Cyan,
        function: Color::Blue,
        constant: Color::Yellow,
        punctuation: Color::Indexed(246),
    },
};

const DRACULA: Theme = Theme {
    name: "dracula",
    base: rgb(0x282a36),
    surface: mix(0x282a36, 0xf8f8f2, SURFACE_PCT),
    highlight: None,
    border: mix(0x282a36, 0x44475a, BORDER_PCT),
    border_focus: rgb(0x44475a),
    faded: rgb(0x6272a4),
    muted: rgb(0x9098bd),
    accent: rgb(0x8be9fd),
    link: rgb(0xbd93f9),
    code: rgb(0xf1fa8c),
    mention: rgb(0xff79c6),
    success: rgb(0x50fa7b),
    warn: rgb(0xffb86c),
    danger: rgb(0xff5555),
    users: [rgb(0x8be9fd), rgb(0x50fa7b), rgb(0xf1fa8c), rgb(0xff79c6), rgb(0xbd93f9), rgb(0xffb86c)],
    syntax: code([0xff79c6, 0xf1fa8c, 0x6272a4, 0xbd93f9, 0x8be9fd, 0x50fa7b, 0xbd93f9, 0x9098bd]),
};

const CATPPUCCIN: Theme = Theme {
    name: "catppuccin",
    base: rgb(0x1e1e2e),
    surface: mix(0x1e1e2e, 0xcdd6f4, SURFACE_PCT),
    highlight: None,
    border: mix(0x1e1e2e, 0x45475a, BORDER_PCT),
    border_focus: rgb(0x45475a),
    faded: rgb(0x6c7086),
    muted: rgb(0xa6adc8),
    accent: rgb(0x89dceb),
    link: rgb(0x89b4fa),
    code: rgb(0xf9e2af),
    mention: rgb(0xcba6f7),
    success: rgb(0xa6e3a1),
    warn: rgb(0xfab387),
    danger: rgb(0xf38ba8),
    users: [rgb(0x89dceb), rgb(0xa6e3a1), rgb(0xf9e2af), rgb(0xf5c2e7), rgb(0x89b4fa), rgb(0xf38ba8)],
    syntax: code([0xcba6f7, 0xa6e3a1, 0x9399b2, 0xfab387, 0xf9e2af, 0x89b4fa, 0xfab387, 0x9399b2]),
};

const CATPPUCCIN_LATTE: Theme = Theme {
    name: "catppuccin-latte",
    base: rgb(0xeff1f5),
    surface: mix(0xeff1f5, 0x4c4f69, SURFACE_PCT),
    highlight: None,
    border: mix(0xeff1f5, 0xbcc0cc, BORDER_PCT),
    border_focus: rgb(0xbcc0cc),
    faded: rgb(0x9ca0b0),
    muted: rgb(0x6c6f85),
    accent: rgb(0x04a5e5),
    link: rgb(0x1e66f5),
    code: rgb(0xdf8e1d),
    mention: rgb(0x8839ef),
    success: rgb(0x40a02b),
    warn: rgb(0xfe640b),
    danger: rgb(0xd20f39),
    users: [rgb(0x04a5e5), rgb(0x40a02b), rgb(0xdf8e1d), rgb(0xea76cb), rgb(0x1e66f5), rgb(0xd20f39)],
    syntax: code([0x8839ef, 0x40a02b, 0x7c7f93, 0xfe640b, 0xdf8e1d, 0x1e66f5, 0xfe640b, 0x7c7f93]),
};

const ROSEPINE: Theme = Theme {
    name: "rosepine",
    base: rgb(0x191724),
    surface: mix(0x191724, 0xe0def4, SURFACE_PCT),
    highlight: None,
    border: mix(0x191724, 0x403d52, BORDER_PCT),
    border_focus: rgb(0x403d52),
    faded: rgb(0x6e6a86),
    muted: rgb(0x908caa),
    accent: rgb(0x9ccfd8),
    link: rgb(0xc4a7e7),
    code: rgb(0xf6c177),
    mention: rgb(0xebbcba),
    success: rgb(0x31748f),
    warn: rgb(0xf6c177),
    danger: rgb(0xeb6f92),
    users: [rgb(0x9ccfd8), rgb(0x31748f), rgb(0xf6c177), rgb(0xeb6f92), rgb(0xc4a7e7), rgb(0xebbcba)],
    syntax: code([0x31748f, 0xf6c177, 0x6e6a86, 0xc4a7e7, 0x9ccfd8, 0xebbcba, 0xc4a7e7, 0x908caa]),
};

const ROSEPINE_DAWN: Theme = Theme {
    name: "rosepine-dawn",
    base: rgb(0xfaf4ed),
    surface: mix(0xfaf4ed, 0x575279, SURFACE_PCT),
    highlight: None,
    border: mix(0xfaf4ed, 0xdfdad9, BORDER_PCT),
    border_focus: rgb(0xdfdad9),
    faded: rgb(0x9893a5),
    muted: rgb(0x797593),
    accent: rgb(0x56949f),
    link: rgb(0x907aa9),
    code: rgb(0xea9d34),
    mention: rgb(0xd7827e),
    success: rgb(0x286983),
    warn: rgb(0xea9d34),
    danger: rgb(0xb4637a),
    users: [rgb(0x56949f), rgb(0x286983), rgb(0xea9d34), rgb(0xb4637a), rgb(0x907aa9), rgb(0xd7827e)],
    syntax: code([0x286983, 0xea9d34, 0x9893a5, 0x907aa9, 0x56949f, 0xd7827e, 0x907aa9, 0x797593]),
};

const NORD: Theme = Theme {
    name: "nord",
    base: rgb(0x2e3440),
    surface: mix(0x2e3440, 0xd8dee9, SURFACE_PCT),
    highlight: None,
    border: mix(0x2e3440, 0x434c5e, BORDER_PCT),
    border_focus: rgb(0x434c5e),
    faded: rgb(0x4c566a),
    muted: rgb(0x7b88a1),
    accent: rgb(0x88c0d0),
    link: rgb(0x81a1c1),
    code: rgb(0xebcb8b),
    mention: rgb(0xb48ead),
    success: rgb(0xa3be8c),
    warn: rgb(0xd08770),
    danger: rgb(0xbf616a),
    users: [rgb(0x88c0d0), rgb(0xa3be8c), rgb(0xebcb8b), rgb(0xb48ead), rgb(0x81a1c1), rgb(0xbf616a)],
    syntax: code([0x81a1c1, 0xa3be8c, 0x616e88, 0xb48ead, 0x8fbcbb, 0x88c0d0, 0xb48ead, 0x81a1c1]),
};

const TOKYONIGHT: Theme = Theme {
    name: "tokyonight",
    base: rgb(0x1a1b26),
    surface: mix(0x1a1b26, 0xc0caf5, SURFACE_PCT),
    highlight: None,
    border: mix(0x1a1b26, 0x3b4261, BORDER_PCT),
    border_focus: rgb(0x3b4261),
    faded: rgb(0x565f89),
    muted: rgb(0x737aa2),
    accent: rgb(0x7dcfff),
    link: rgb(0x7aa2f7),
    code: rgb(0xe0af68),
    mention: rgb(0xbb9af7),
    success: rgb(0x9ece6a),
    warn: rgb(0xff9e64),
    danger: rgb(0xf7768e),
    users: [rgb(0x7dcfff), rgb(0x9ece6a), rgb(0xe0af68), rgb(0xbb9af7), rgb(0x7aa2f7), rgb(0xf7768e)],
    syntax: code([0xbb9af7, 0x9ece6a, 0x565f89, 0xff9e64, 0x2ac3de, 0x7aa2f7, 0xff9e64, 0x89ddff]),
};

const MONOKAI: Theme = Theme {
    name: "monokai",
    base: rgb(0x272822),
    surface: mix(0x272822, 0xf8f8f2, SURFACE_PCT),
    highlight: None,
    border: mix(0x272822, 0x49483e, BORDER_PCT),
    border_focus: rgb(0x49483e),
    faded: rgb(0x75715e),
    muted: rgb(0xa59f85),
    accent: rgb(0x66d9ef),
    link: rgb(0xae81ff),
    code: rgb(0xe6db74),
    mention: rgb(0xf92672),
    success: rgb(0xa6e22e),
    warn: rgb(0xfd971f),
    danger: rgb(0xf92672),
    users: [rgb(0x66d9ef), rgb(0xa6e22e), rgb(0xe6db74), rgb(0xf92672), rgb(0xae81ff), rgb(0xfd971f)],
    syntax: code([0xf92672, 0xe6db74, 0x75715e, 0xae81ff, 0x66d9ef, 0xa6e22e, 0xae81ff, 0xa59f85]),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_name_resolves_to_itself() {
        for name in Theme::NAMES {
            assert_eq!(Theme::named(name).map(|t| t.name), Some(name), "{name}");
        }
    }

    #[test]
    fn names_are_forgiving_and_unknown_is_none() {
        assert_eq!(Theme::named("Tokyo Night").unwrap().name, "tokyonight");
        assert_eq!(Theme::named("catppuccin_mocha").unwrap().name, "catppuccin");
        assert_eq!(Theme::named("Rose-Pine Dawn").unwrap().name, "rosepine-dawn");
        assert_eq!(Theme::named("solarized"), None);
        assert_eq!(Theme::named(""), None);
    }

    #[test]
    fn mix_walks_each_channel_toward_the_target() {
        assert_eq!(mix(0x000000, 0xffffff, 50), Color::Rgb(127, 127, 127));
        assert_eq!(mix(0x1e1e2e, 0xcdd6f4, 0), rgb(0x1e1e2e));
        assert_eq!(mix(0x1e1e2e, 0xcdd6f4, 100), rgb(0xcdd6f4));
        assert_eq!(mix(0x102030, 0x304050, 10), Color::Rgb(0x13, 0x23, 0x33));
    }

    #[test]
    fn every_frame_sits_nearer_the_ground_than_the_focused_one_and_never_in_the_accent() {
        let distance = |a: Color, b: Color| match (a, b) {
            (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
                u32::from(r1.abs_diff(r2)) + u32::from(g1.abs_diff(g2)) + u32::from(b1.abs_diff(b2))
            }
            _ => 0,
        };
        for name in Theme::NAMES {
            let theme = Theme::named(name).unwrap();
            assert_ne!(theme.border_focus, theme.accent, "{name}");
            if matches!(theme.base, Color::Rgb(..)) {
                assert!(distance(theme.border, theme.base) < distance(theme.border_focus, theme.base), "{name}");
            }
        }
    }

    #[test]
    fn a_known_ground_replaces_the_default_ground_and_leaves_rgb_themes_alone() {
        assert_eq!(Theme::default().with_ground(0x1e1e2e).base, rgb(0x1e1e2e));
        let nord = Theme::named("nord").unwrap();
        assert_eq!(nord.with_ground(0xffffff), nord);
    }

    #[test]
    fn user_colors_are_stable_and_spread() {
        let theme = Theme::default();
        assert_eq!(theme.user("nina"), theme.user(" nina "));
        let distinct: std::collections::HashSet<_> =
            ["a", "b", "c", "d", "e", "f", "g", "h"].iter().map(|n| format!("{:?}", theme.user(n))).collect();
        assert!(distinct.len() > 1);
    }

    #[test]
    fn colour_returns_the_field_of_each_token() {
        let syntax = Syntax {
            keyword: Color::Indexed(1),
            string: Color::Indexed(2),
            comment: Color::Indexed(3),
            number: Color::Indexed(4),
            kind: Color::Indexed(5),
            function: Color::Indexed(6),
            constant: Color::Indexed(7),
            punctuation: Color::Indexed(8),
        };
        let tokens = [
            Token::Keyword,
            Token::String,
            Token::Comment,
            Token::Number,
            Token::Type,
            Token::Function,
            Token::Constant,
            Token::Punctuation,
        ];
        let colours: Vec<Color> = tokens.into_iter().map(|token| syntax.colour(token)).collect();
        assert_eq!(colours, (1..=8).map(Color::Indexed).collect::<Vec<_>>());
    }

    #[test]
    fn every_palette_draws_code_in_colours_that_differ_from_its_ground() {
        for name in Theme::NAMES {
            let theme = Theme::named(name).unwrap();
            let s = theme.syntax;
            for colour in [s.keyword, s.string, s.comment, s.number, s.kind, s.function, s.constant, s.punctuation] {
                assert_ne!(colour, theme.base, "{name}");
            }
        }
    }

    #[test]
    fn the_default_palette_colours_code_with_the_terminal_ansi_colours() {
        let expected = Syntax {
            keyword: Color::Magenta,
            string: Color::Green,
            comment: Color::Indexed(244),
            number: Color::Yellow,
            kind: Color::Cyan,
            function: Color::Blue,
            constant: Color::Yellow,
            punctuation: Color::Indexed(246),
        };
        assert_eq!(Theme::default().syntax, expected);
    }
}
