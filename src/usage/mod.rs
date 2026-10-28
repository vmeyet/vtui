//! Usage counts an app keeps on the user's machine, opt-in: how often each action ran, how long each screen was read,
//! and how often a habit showed that a faster key replaces.
//! Only names are counted, the ones the app hands in through [`Rules`]: nothing about what the user worked on.
//! The counts live in a JSON-lines file, one line per local day; this module turns that text into [`Day`]s and back,
//! and the app reads and writes the file where it keeps its cache.
mod clock;
mod report;

pub use clock::Clock;
pub use report::{BadWindow, Count, Hinted, RARELY, Report, Time, Unused, Window, duration};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::time::Duration;

/// How often a hint shows as a toast, over every session; after that only the report counts it.
const HINT_TOASTS: u64 = 3;

/// The counting a line without a `version` was made with: the first one.
const FIRST_COUNTING: u32 = 1;

/// What the app counts and how it names it; vtui knows none of these names.
#[derive(Clone, Debug)]
pub struct Rules {
    /// Every action the report lists even when unused, in the order RARELY USED lists them.
    pub actions: Vec<&'static str>,
    /// The groups NEVER USED sorts unused actions into, in the order it shows them; an action no group lists goes
    /// under `other`.
    pub groups: Vec<Group>,
    /// The habits a faster key replaces, in the order HINTS lists them.
    pub hints: &'static [Hint],
    /// Action names an older version of the app wrote, each counted under its new name.
    pub renamed: &'static [Renamed],
    /// How the app counts time and hints now, from 1; raise it when that changes, and the days counted before keep
    /// only their actions in the report.
    pub counting: u32,
}

/// A heading of NEVER USED and the actions under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// The heading, as the report prints it.
    pub title: &'static str,
    /// Its actions, in the order it lists them.
    pub actions: Vec<&'static str>,
}

/// A habit a faster key replaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hint {
    /// The name it is counted under.
    pub name: &'static str,
    /// What the user did and the key to try instead, as the report prints it.
    pub text: &'static str,
}

/// An action that changed its name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Renamed {
    /// The name an older version wrote.
    pub old: &'static str,
    /// The name it has now.
    pub new: &'static str,
}

impl Rules {
    /// The days of a usage file, oldest first, each action under its current name; a line that does not parse is
    /// skipped rather than failing.
    pub fn parse(&self, text: &str) -> Vec<Day> {
        let mut days: Vec<Day> = text
            .lines()
            .filter_map(|line| serde_json::from_str::<Day>(line).ok())
            .map(|day| Day { counts: Counts { actions: self.current_names(&day.counts.actions), ..day.counts }, ..day })
            .collect();
        days.sort_by_key(|day| day.date);
        days
    }

    /// The usage file with `counts` added to `date`'s line: one line per day, however often it is merged.
    pub fn merge(&self, text: &str, date: NaiveDate, counts: &Counts) -> Result<String, serde_json::Error> {
        let mut days = self.parse(text);
        let index = days.iter().position(|day| day.date == date).unwrap_or_else(|| {
            days.push(Day { date, version: self.counting, counts: Counts::default() });
            days.len() - 1
        });
        days[index].counts.add(counts);
        days.sort_by_key(|day| day.date);
        days.iter().map(|day| serde_json::to_string(day).map(|line| line + "\n")).collect()
    }

    fn current_names(&self, actions: &BTreeMap<String, u64>) -> BTreeMap<String, u64> {
        let mut current = BTreeMap::new();
        for (name, count) in actions {
            *current.entry(self.current_name(name).to_owned()).or_default() += count;
        }
        current
    }

    fn current_name<'a>(&self, name: &'a str) -> &'a str {
        self.renamed.iter().find(|renamed| renamed.old == name).map_or(name, |renamed| renamed.new)
    }
}

/// What one session counted since the last flush, kept in memory; the app writes it out now and then.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    actions: BTreeMap<&'static str, u64>,
    screens_ms: BTreeMap<&'static str, u64>,
    hints: BTreeMap<&'static str, u64>,
    shown: BTreeMap<&'static str, u64>,
    shown_ever: BTreeMap<String, u64>,
}

impl Tally {
    /// A session's tally, knowing how often each hint was shown in the sessions before.
    pub fn after(days: &[Day]) -> Self {
        let mut shown_ever = BTreeMap::new();
        for day in days {
            add(&mut shown_ever, &day.counts.shown);
        }
        Self { shown_ever, ..Self::default() }
    }

    /// Counts one run of the action `name`.
    pub fn act(&mut self, name: &'static str) {
        *self.actions.entry(name).or_default() += 1;
    }

    /// Counts the habit `name`, and says whether to show its hint: yes for its first few times, every session counted.
    pub fn hint(&mut self, name: &'static str) -> bool {
        *self.hints.entry(name).or_default() += 1;
        let shown = self.shown_ever.entry(name.to_owned()).or_default();
        let is_shown = *shown < HINT_TOASTS;
        if is_shown {
            *shown += 1;
            *self.shown.entry(name).or_default() += 1;
        }
        is_shown
    }

    /// Adds `time` read on `screen`.
    pub fn spend(&mut self, screen: &'static str, time: Duration) {
        *self.screens_ms.entry(screen).or_default() += u64::try_from(time.as_millis()).unwrap_or(u64::MAX);
    }

    /// The counts to write, leaving behind the part of a second not yet spent on each screen; `None` when there is
    /// nothing to write.
    pub fn take(&mut self) -> Option<Counts> {
        let screens = self.screens_ms.iter().filter(|(_, ms)| **ms >= 1000).map(|(screen, ms)| ((*screen).to_owned(), ms / 1000)).collect();
        for ms in self.screens_ms.values_mut() {
            *ms %= 1000;
        }
        let counts = Counts { actions: owned(&mut self.actions), screens, hints: owned(&mut self.hints), shown: owned(&mut self.shown) };
        let is_empty = counts.actions.is_empty() && counts.screens.is_empty() && counts.hints.is_empty();
        (!is_empty).then_some(counts)
    }
}

fn owned(counts: &mut BTreeMap<&'static str, u64>) -> BTreeMap<String, u64> {
    std::mem::take(counts).into_iter().map(|(name, count)| (name.to_owned(), count)).collect()
}

/// What a flush writes, and what a day holds: whole seconds only, the milliseconds below a second wait for the next one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    /// Runs per action.
    #[serde(default)]
    pub actions: BTreeMap<String, u64>,
    /// Seconds per screen.
    #[serde(default)]
    pub screens: BTreeMap<String, u64>,
    /// Times per habit.
    #[serde(default)]
    pub hints: BTreeMap<String, u64>,
    /// Times per hint shown as a toast.
    #[serde(default)]
    pub shown: BTreeMap<String, u64>,
}

impl Counts {
    fn add(&mut self, more: &Self) {
        add(&mut self.actions, &more.actions);
        add(&mut self.screens, &more.screens);
        add(&mut self.hints, &more.hints);
        add(&mut self.shown, &more.shown);
    }
}

/// One line of the usage file: everything one local day counted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Day {
    /// The local day.
    pub date: NaiveDate,
    /// The oldest [`Rules::counting`] that went into the day; a line from before versions has none, and reads as 1.
    #[serde(default = "first_counting")]
    pub version: u32,
    /// What it counted, its maps written beside the date as they always were.
    #[serde(flatten)]
    pub counts: Counts,
}

fn first_counting() -> u32 {
    FIRST_COUNTING
}

/// Adds each count of `from` to `into`.
fn add(into: &mut BTreeMap<String, u64>, from: &BTreeMap<String, u64>) {
    for (name, count) in from {
        *into.entry(name.clone()).or_default() += count;
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;

    pub(crate) const HINTS: &[Hint] = &[Hint { name: "long_walk", text: "walked 15 lines; ]c jumps to the next hunk" }];

    pub(crate) fn rules() -> Rules {
        Rules {
            actions: vec!["move_down", "next_thread", "zen", "merge", "react", "side_by_side"],
            groups: vec![
                Group { title: "move", actions: vec!["move_down", "next_thread"] },
                Group { title: "view", actions: vec!["zen", "side_by_side"] },
            ],
            hints: HINTS,
            renamed: &[Renamed { old: "split", new: "side_by_side" }],
            counting: 2,
        }
    }

    pub(crate) fn date(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::fixtures::{date, rules};
    use super::*;

    fn acted(name: &str, count: u64) -> Counts {
        Counts { actions: BTreeMap::from([(name.to_owned(), count)]), ..Counts::default() }
    }

    #[test]
    fn a_tally_with_nothing_counted_has_nothing_to_write() {
        assert_eq!(Tally::default().take(), None);
    }

    #[test]
    fn a_tally_hands_whole_seconds_and_keeps_the_rest_for_the_next_flush() {
        let mut tally = Tally::default();
        tally.act("next_thread");
        tally.spend("diff", Duration::from_millis(2500));
        let counts = tally.take().unwrap();
        assert_eq!((counts.actions["next_thread"], counts.screens["diff"]), (1, 2));
        tally.spend("diff", Duration::from_millis(600));
        assert_eq!(tally.take().unwrap().screens["diff"], 1);
    }

    #[test]
    fn merging_twice_into_one_day_keeps_one_line_and_adds() {
        let once = rules().merge("", date("2026-09-24"), &acted("zen", 2)).unwrap();
        let twice = rules().merge(&once, date("2026-09-24"), &acted("zen", 2)).unwrap();
        assert_eq!(twice.lines().count(), 1);
        assert_eq!(rules().parse(&twice)[0].counts.actions["zen"], 4);
    }

    #[test]
    fn merging_an_earlier_day_keeps_the_days_oldest_first() {
        let later = rules().merge("", date("2026-09-24"), &acted("zen", 1)).unwrap();
        let both = rules().merge(&later, date("2026-09-23"), &acted("zen", 1)).unwrap();
        let dates: Vec<NaiveDate> = rules().parse(&both).iter().map(|day| day.date).collect();
        assert_eq!(dates, [date("2026-09-23"), date("2026-09-24")]);
    }

    #[test]
    fn an_empty_merge_changes_nothing() {
        let text = rules().merge("", date("2026-09-24"), &acted("zen", 1)).unwrap();
        assert_eq!(rules().merge(&text, date("2026-09-24"), &Counts::default()).unwrap(), text);
    }

    #[test]
    fn a_new_day_is_written_with_the_counting_of_the_rules() {
        let text = rules().merge("", date("2026-09-24"), &acted("zen", 1)).unwrap();
        assert_eq!(text, "{\"date\":\"2026-09-24\",\"version\":2,\"actions\":{\"zen\":1},\"screens\":{},\"hints\":{},\"shown\":{}}\n");
    }

    #[test]
    fn a_line_without_a_version_reads_as_the_first_counting() {
        assert_eq!(rules().parse("{\"date\":\"2026-09-24\"}")[0].version, 1);
    }

    #[test]
    fn a_broken_line_is_skipped() {
        let text = "{not json}\n{\"date\":\"2026-09-24\",\"actions\":{\"zen\":1}}\n";
        assert_eq!(rules().parse(text).len(), 1);
    }

    #[test]
    fn a_renamed_action_reads_under_its_new_name_added_to_it() {
        let day = &rules().parse("{\"date\":\"2026-09-24\",\"actions\":{\"split\":1,\"side_by_side\":2}}")[0];
        assert_eq!(day.counts.actions, BTreeMap::from([("side_by_side".to_owned(), 3)]));
    }

    #[test]
    fn a_hint_shows_its_first_times_over_every_session_then_only_counts() {
        let mut first = Tally::default();
        assert_eq!([first.hint("long_walk"), first.hint("long_walk")], [true, true]);
        let text = rules().merge("", date("2026-09-23"), &first.take().unwrap()).unwrap();
        let mut next = Tally::after(&rules().parse(&text));
        assert_eq!([next.hint("long_walk"), next.hint("long_walk"), next.hint("palette_number")], [true, false, true]);
        let counts = next.take().unwrap();
        assert_eq!((counts.hints["long_walk"], counts.shown["long_walk"]), (2, 1));
    }
}
