# Architecture

`vtui` is one library crate: the terminal UI pieces an app needs and that know nothing about the app.
This page says how the code is laid out today.
When a module or a boundary changes, change this page in the same change.

## The rule

vtui knows no app.
No module names an app's type, action, key map, pane or event loop.
Where a piece needs something only the app knows, it takes it as a value or a type parameter: the palette's modes are the app's type implementing `palette::Mode`, the help overlay draws the `help::Section`s the app builds, the palette view lists the `palette::view::Pick`s the app hands it.
If a move from an app would need vtui to know an app type, the piece is made generic or stays in the app.

## Module map

| Module | What it holds |
|---|---|
| `screen` | `Screen`: raw mode on the alternate screen, the mouse, focus changes, bracketed paste so a paste arrives as one `Event::Paste`, and the kitty modifier report; `enter_without_mouse` leaves the mouse to the terminal so its text selection keeps working; `lend` hands the terminal to `$EDITOR` or another program and takes it back |
| `ground` | `ground::ask`: the terminal's background colour through OSC 11, read against a deadline |
| `theme` | `Theme`: the colour tokens every pane draws with, nine named palettes, `rgb` and `mix` for an app's own colours, `with_ground` for the default palette; `Syntax` gives each code `Token` its colour in every palette |
| `pane` | `pane::framed`: the rounded frame of a pane or modal, brighter when it has the keys; `pane::fade` dims what is drawn behind a modal |
| `text` | `truncate`, `truncate_middle`, `truncate_tabs`: text cut to a number of terminal columns; `truncate_line`: a styled line cut the same way, each span keeping its style |
| `field` | `Field`: the input box, its text and cursor in one value, the word under the cursor, a paste cleaned and typed as one edit, and a `revision` that grows with every change to the text |
| `fuzzy` | fzf-style subsequence ranking, the single obvious match, and suggestions for a mistyped word |
| `toward` | `Toward`: which way a move goes through a list that wraps around |
| `complete` | Tab completion for any input box: `Cycle` through the candidates and the `ghost` of the best one |
| `palette` | `Palette<M>`: the `ctrl-k` line, generic over the app's `Mode`, a paste typed on its one line; `Verb` and `verbs_for` for its command list; `view` draws the popup, each `Pick` a row whose styled detail, such as badges, is cut only after the label |
| `editor` | `editor::edit`: long text written in `$VISUAL` or `$EDITOR` through a temp file, `Edited` says how it came back |
| `help` | `Help` and `draw`: the `?` overlay over the `Section`s the app builds, in one or two columns |
| `clipboard` | `clipboard::print`: the terminal's clipboard through OSC 52, wrapped for tmux |
| `motion` | `Motion`: how often the screen moves on its own, lively or calm |
| `log` | `OrWarn`: a failed terminal write logged through `tracing`, never dropped (private) |

## Boundaries

- Side effects live in `screen`, `ground`, `clipboard` and `editor`: the first three write to the terminal or read `/dev/tty`, `editor` writes a temp file and runs the user's editor.
- Every other module is pure: values in, values or `ratatui` lines out.
- Drawing goes through a `ratatui::Frame` the app hands in; vtui never owns the terminal loop.
- Errors of best-effort terminal writes go to `tracing`; the app decides where the log goes.
