# Working in this repo

`vtui` is a Rust library of terminal UI building blocks shared by the vmeyet TUIs (revu first).
Read [`ARCHITECTURE.md`](ARCHITECTURE.md) first: the module map and the one rule that shapes everything, vtui knows no app.
The code is the contract; `ARCHITECTURE.md` describes it, and a change to a module or a boundary updates it in the same change.

## Ground rules

- One change a reviewer can read in one sitting, under 500 changed lines excluding fixtures.
- Branches: `<scope>/<kebab-title>` with scope in `feat fix chore refactor docs test ci`; worktrees under `.claude/worktrees/<scope>+<kebab-title>`.
- Commits follow conventional commits; body in two short imperative sentences at most.
- Markdown files put one sentence per line.
- A release is a tag `vX.Y.Z` matching `version` in `Cargo.toml`; apps depend on the tag through git.

## Code

- Rust 2024, `rustfmt.toml` as committed (140 columns, `use_small_heuristics = "Max"`).
- Lints live in `[lints]` in `Cargo.toml`: `cargo clippy --all-targets -- -D warnings` must be clean, and `unwrap`/`expect` only live in tests.
- `pub` is the crate's API: keep it to what an app calls, everything else private or `pub(crate)`; `missing_docs` holds every `pub` item to a doc comment.
- Doc comments (`///`, `//!`) say what an item is for or why it is so, and stay true when it changes.
- No comment restates the code; a `//` comment says why, and only when no name can.
- Immutability across function boundaries: pass helpers values or `&` borrows and take back a new value; only the owner of a state changes it, through `&mut self`.
- Flat bodies of named steps at one level of abstraction.
- Design a signature from the call site: a caller never needs to read the body to use it right.
- Never drop a `Result` silently: best-effort terminal writes go through `log::OrWarn`, which logs with `tracing::warn!`.
- No `bool` parameter on a function with more than one caller; no tuple for data that crosses a module: a struct with named fields.
- `unsafe` only inside a safe function whose runtime checks make its `SAFETY:` comment true.
- One crate per job: prefer std or a dependency already in `Cargo.toml`; a new one needs a line saying why.
- About 800 lines is the guideline for a file; a module with children lives in `foo/mod.rs`.

## Tests

```sh
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo deny check
cargo machete
```

Every module ships its tests in the same file, one behaviour per test, named as a sentence that says it.
Tests are deterministic: no real terminal, no real network, no real clock.
Code that talks to the terminal keeps its parsing apart, so the test drives the parser with bytes.
Fixtures are invented: user `nina`, project `acme/widgets`.

## Security

- The repo is public: no token, no real host other than `gitlab.com` and `github.com`, no real project path, no work email.
- No telemetry and no crash reporting.
