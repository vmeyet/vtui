//! An app's `update` command and its `--version` label: brew upgrades a binary brew installed, cargo rebuilds any other
//! from the app's repo, unless `git ls-remote` shows the running commit is already the latest.
//! Each app hands in its own [`App`], so no app needs another one installed.
use std::fmt;
use std::io;
use std::path::Path;
use std::process::Command;

/// The running app and where a newer one comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct App {
    /// The release number, as `env!("CARGO_PKG_VERSION")`.
    pub version: &'static str,
    /// The full hash of the commit the binary was built from, as a `build.rs` embeds it; empty or anything else when
    /// built without git history.
    pub commit: &'static str,
    /// The git repo cargo builds from: `https://github.com/acme/widgets`.
    pub repo: &'static str,
    /// The Homebrew formula brew upgrades: `acme/tap/widgets`.
    pub formula: &'static str,
}

/// Whether `update` installs over the latest version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reinstall {
    /// Install only when the running binary may be behind.
    WhenBehind,
    /// Install even when the running binary is the latest: `brew reinstall`, or a cargo build without the check.
    Always,
}

/// What `update` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// It installed the latest version.
    Install,
    /// The running binary is the latest, so nothing ran.
    UpToDate,
}

/// What `update` tells the app as it goes, for the app to say in its own words.
#[derive(Clone, Copy, Debug)]
pub enum Step<'a> {
    /// The latest commit could not be asked; the update installs anyway.
    CheckFailed(&'a io::Error),
    /// The install is about to run, as this command line.
    Installing(&'a Call),
}

/// A program and its arguments, shown as typed in a shell: `brew upgrade acme/tap/widgets`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    program: &'static str,
    args: Vec<&'static str>,
}

impl fmt::Display for Call {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.program, self.args.join(" "))
    }
}

/// A git commit, as its full hash of 40 hex characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Commit<'a>(&'a str);

impl<'a> Commit<'a> {
    fn parse(text: &'a str) -> Option<Self> {
        let is_full_hash = text.len() == 40 && text.bytes().all(|b| b.is_ascii_hexdigit());
        is_full_hash.then_some(Self(text))
    }

    /// The first seven characters, as git shows a commit.
    fn short(self) -> &'a str {
        &self.0[..7]
    }
}

/// Who installed the running binary, and so who updates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Installer {
    Brew,
    Cargo,
}

impl Installer {
    /// Cargo when the binary's path cannot be read, since cargo can always build the app again.
    fn running() -> Self {
        std::env::current_exe().and_then(std::fs::canonicalize).map_or(Self::Cargo, |exe| Self::of(&exe))
    }

    /// Who installed the binary at `exe`, a path with its links resolved.
    fn of(exe: &Path) -> Self {
        if exe.components().any(|part| part.as_os_str() == "Cellar") { Self::Brew } else { Self::Cargo }
    }

    /// brew ships tagged releases only, so the latest commit says nothing about what brew would install.
    fn checks_the_repo(self, reinstall: Reinstall) -> bool {
        self == Self::Cargo && reinstall == Reinstall::WhenBehind
    }
}

/// Runs the programs an update needs; tests answer in their place, so no test runs brew, cargo or git.
trait Runner {
    /// Runs `call` on the terminal, and fails when it does.
    fn run(&mut self, call: &Call) -> io::Result<()>;
    /// Runs `call` and returns what it printed.
    fn read(&mut self, call: &Call) -> io::Result<String>;
}

/// The programs themselves.
#[derive(Debug)]
struct System;

impl Runner for System {
    fn run(&mut self, call: &Call) -> io::Result<()> {
        let status = command(call).status().map_err(|err| not_run(call, &err))?;
        if !status.success() {
            return Err(io::Error::other(format!("`{call}` failed")));
        }
        Ok(())
    }

    fn read(&mut self, call: &Call) -> io::Result<String> {
        let output = command(call).output().map_err(|err| not_run(call, &err))?;
        if !output.status.success() {
            return Err(io::Error::other(format!("`{call}` failed: {}", String::from_utf8_lossy(&output.stderr).trim())));
        }
        String::from_utf8(output.stdout).map_err(io::Error::other)
    }
}

fn command(call: &Call) -> Command {
    let mut command = Command::new(call.program);
    // git, and cargo through it, would otherwise wait on a credentials prompt.
    command.args(&call.args).env("GIT_TERMINAL_PROMPT", "0");
    command
}

fn not_run(call: &Call, err: &io::Error) -> io::Error {
    io::Error::new(err.kind(), format!("could not run {}: {err}", call.program))
}

/// Only a running commit known to be the latest spares the install; anything unanswered installs.
fn decide(running: Option<Commit<'_>>, latest: Option<Commit<'_>>) -> Decision {
    match (running, latest) {
        (Some(running), Some(latest)) if running == latest => Decision::UpToDate,
        _ => Decision::Install,
    }
}

/// The commit `git ls-remote <repo> HEAD` printed, as `<hash>\tHEAD`.
fn latest_in(listing: &str) -> Option<Commit<'_>> {
    listing.split_whitespace().next().and_then(Commit::parse)
}

fn call(program: &'static str, args: &[&'static str]) -> Call {
    Call { program, args: args.to_vec() }
}

impl App {
    /// `0.1.0 (a1b2c3d)`, as `--version` shows it; the number alone when built without git history.
    pub fn label(&self) -> String {
        match Commit::parse(self.commit) {
            Some(commit) => format!("{} ({})", self.version, commit.short()),
            None => self.version.to_owned(),
        }
    }

    /// Installs the latest version, with brew when brew installed the running binary, else built with cargo from the repo,
    /// unless the repo's latest commit is the running one; `report` hears each [`Step`] as it happens.
    /// It fails when the install does, never when only the check does.
    pub fn update(&self, reinstall: Reinstall, report: impl FnMut(Step<'_>)) -> io::Result<Decision> {
        self.update_with(&mut System, Installer::running(), reinstall, report)
    }

    fn update_with(
        &self,
        runner: &mut impl Runner,
        installer: Installer,
        reinstall: Reinstall,
        mut report: impl FnMut(Step<'_>),
    ) -> io::Result<Decision> {
        let decision = if installer.checks_the_repo(reinstall) { self.check(runner, &mut report) } else { Decision::Install };
        if decision == Decision::Install {
            let install = self.install(installer, reinstall);
            report(Step::Installing(&install));
            runner.run(&install)?;
        }
        Ok(decision)
    }

    fn check(&self, runner: &mut impl Runner, report: &mut impl FnMut(Step<'_>)) -> Decision {
        match runner.read(&self.latest_check()) {
            Ok(listing) => decide(Commit::parse(self.commit), latest_in(&listing)),
            Err(err) => {
                report(Step::CheckFailed(&err));
                Decision::Install
            }
        }
    }

    fn latest_check(&self) -> Call {
        call("git", &["ls-remote", self.repo, "HEAD"])
    }

    fn install(&self, installer: Installer, reinstall: Reinstall) -> Call {
        match (installer, reinstall) {
            (Installer::Brew, Reinstall::WhenBehind) => call("brew", &["upgrade", self.formula]),
            (Installer::Brew, Reinstall::Always) => call("brew", &["reinstall", self.formula]),
            (Installer::Cargo, _) => call("cargo", &["install", "--git", self.repo, "--locked", "--force"]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNNING: &str = "9731436a0e7c4d1b2f3a4b5c6d7e8f9a0b1c2d3e";
    const NEWER: &str = "635cf1b0000000000000000000000000000000ff";
    const CHECK: &str = "git ls-remote https://github.com/acme/widgets HEAD";
    const CARGO_INSTALL: &str = "cargo install --git https://github.com/acme/widgets --locked --force";

    const WIDGETS: App = App { version: "0.1.0", commit: RUNNING, repo: "https://github.com/acme/widgets", formula: "acme/tap/widgets" };

    /// Answers the repo check with `listing`, or fails it when `None`, and keeps every call in `ran`.
    #[derive(Debug, Default)]
    struct Fake {
        listing: Option<String>,
        install_fails: bool,
        ran: Vec<String>,
    }

    impl Fake {
        fn answering(latest: &str) -> Self {
            Self { listing: Some(format!("{latest}\tHEAD\n")), ..Self::default() }
        }
    }

    impl Runner for Fake {
        fn run(&mut self, call: &Call) -> io::Result<()> {
            self.ran.push(call.to_string());
            if self.install_fails { Err(io::Error::other(format!("`{call}` failed"))) } else { Ok(()) }
        }

        fn read(&mut self, call: &Call) -> io::Result<String> {
            self.ran.push(call.to_string());
            self.listing.clone().ok_or_else(|| io::Error::other("offline"))
        }
    }

    fn update(fake: &mut Fake, installer: Installer, reinstall: Reinstall) -> io::Result<Decision> {
        WIDGETS.update_with(fake, installer, reinstall, |_| {})
    }

    #[test]
    fn a_binary_in_the_brew_cellar_is_installed_by_brew() {
        assert_eq!(Installer::of(Path::new("/opt/homebrew/Cellar/widgets/0.1.0/bin/widgets")), Installer::Brew);
    }

    #[test]
    fn a_binary_built_by_cargo_is_installed_by_cargo() {
        assert_eq!(Installer::of(Path::new("/Users/nina/.cargo/bin/widgets")), Installer::Cargo);
    }

    #[test]
    fn a_cargo_install_on_the_latest_commit_installs_nothing() {
        let mut fake = Fake::answering(RUNNING);
        let decision = update(&mut fake, Installer::Cargo, Reinstall::WhenBehind).unwrap();
        assert_eq!((decision, fake.ran), (Decision::UpToDate, vec![CHECK.to_owned()]));
    }

    #[test]
    fn a_cargo_install_behind_the_repo_builds_it_with_its_lockfile() {
        let mut fake = Fake::answering(NEWER);
        let decision = update(&mut fake, Installer::Cargo, Reinstall::WhenBehind).unwrap();
        assert_eq!((decision, fake.ran), (Decision::Install, vec![CHECK.to_owned(), CARGO_INSTALL.to_owned()]));
    }

    #[test]
    fn a_failed_check_installs_anyway_and_says_so() {
        let mut fake = Fake::default();
        let mut has_check_failed = false;
        let decision = WIDGETS
            .update_with(&mut fake, Installer::Cargo, Reinstall::WhenBehind, |step| {
                has_check_failed |= matches!(step, Step::CheckFailed(_));
            })
            .unwrap();
        assert_eq!((decision, has_check_failed), (Decision::Install, true));
    }

    #[test]
    fn the_install_is_reported_before_it_runs() {
        let mut fake = Fake::answering(NEWER);
        let mut installs = Vec::new();
        WIDGETS
            .update_with(&mut fake, Installer::Cargo, Reinstall::WhenBehind, |step| {
                if let Step::Installing(call) = step {
                    installs.push(call.to_string());
                }
            })
            .unwrap();
        assert_eq!(installs, [CARGO_INSTALL]);
    }

    #[test]
    fn a_binary_built_without_git_history_installs() {
        let mut fake = Fake::answering(NEWER);
        let unknown = App { commit: "", ..WIDGETS };
        let decision = unknown.update_with(&mut fake, Installer::Cargo, Reinstall::WhenBehind, |_| {}).unwrap();
        assert_eq!(decision, Decision::Install);
    }

    #[test]
    fn an_empty_listing_installs() {
        let mut fake = Fake { listing: Some(String::new()), ..Fake::default() };
        assert_eq!(update(&mut fake, Installer::Cargo, Reinstall::WhenBehind).unwrap(), Decision::Install);
    }

    #[test]
    fn always_builds_without_asking_the_repo() {
        let mut fake = Fake::answering(RUNNING);
        update(&mut fake, Installer::Cargo, Reinstall::Always).unwrap();
        assert_eq!(fake.ran, [CARGO_INSTALL]);
    }

    #[test]
    fn a_brew_install_upgrades_without_asking_the_repo() {
        let mut fake = Fake::answering(RUNNING);
        update(&mut fake, Installer::Brew, Reinstall::WhenBehind).unwrap();
        assert_eq!(fake.ran, ["brew upgrade acme/tap/widgets"]);
    }

    #[test]
    fn a_brew_install_reinstalls_when_asked_to_always_install() {
        let mut fake = Fake::default();
        update(&mut fake, Installer::Brew, Reinstall::Always).unwrap();
        assert_eq!(fake.ran, ["brew reinstall acme/tap/widgets"]);
    }

    #[test]
    fn a_failed_install_fails_the_update() {
        let mut fake = Fake { install_fails: true, ..Fake::answering(NEWER) };
        assert!(update(&mut fake, Installer::Cargo, Reinstall::WhenBehind).is_err());
    }

    #[test]
    fn the_label_shows_the_short_commit_after_the_number() {
        assert_eq!(WIDGETS.label(), "0.1.0 (9731436)");
    }

    #[test]
    fn the_label_is_the_number_alone_without_a_commit() {
        assert_eq!(App { commit: "", ..WIDGETS }.label(), "0.1.0");
    }

    #[test]
    fn a_short_or_non_hex_commit_is_no_commit() {
        assert_eq!(App { commit: "9731436", ..WIDGETS }.label(), "0.1.0");
        assert_eq!(App { commit: "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", ..WIDGETS }.label(), "0.1.0");
    }
}
