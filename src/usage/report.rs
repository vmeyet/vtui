//! The usage file's days summed over a window: what was never used, rarely, most, where the time went, which habits
//! showed.
use super::{Day, Rules, add};
use chrono::NaiveDate;
use serde::{Serialize, Serializer};
use std::collections::BTreeMap;
use std::str::FromStr;

/// An action used this often or less is listed under RARELY USED.
pub const RARELY: u64 = 2;
/// MOST USED lists this many actions.
const MOST: usize = 10;

/// How far back a report looks: `30d`, `4w`, `all`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    /// The last this many days, today included.
    Days(u32),
    /// Every recorded day.
    All,
}

/// A window that is neither a number of days or weeks nor `all`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadWindow;

impl std::fmt::Display for BadWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("takes a number of days or weeks, like 30d or 4w, or all")
    }
}

impl std::error::Error for BadWindow {}

impl FromStr for Window {
    type Err = BadWindow;

    fn from_str(text: &str) -> Result<Self, BadWindow> {
        if text == "all" {
            return Ok(Self::All);
        }
        let days = |number: &str| number.parse::<u32>().ok();
        let window = match (text.strip_suffix('d'), text.strip_suffix('w')) {
            (Some(number), _) => days(number),
            (_, Some(number)) => days(number).map(|weeks| weeks.saturating_mul(7)),
            _ => None,
        };
        window.map(Self::Days).ok_or(BadWindow)
    }
}

impl Window {
    fn holds(self, date: NaiveDate, today: NaiveDate) -> bool {
        match self {
            Self::Days(days) => (today - date).num_days() < i64::from(days),
            Self::All => true,
        }
    }
}

/// The days of a window, summed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The days recorded in the window.
    pub days: usize,
    /// Those counted before the app's current [`Rules::counting`]: their time and hints are left out.
    pub old_days: usize,
    /// Every action the rules list and the window never counted, by group.
    pub never: Vec<Unused>,
    /// The actions the rules list that the window counted only a few times, in the rules' order.
    pub rarely: Vec<Count>,
    /// The actions counted most, most first.
    pub most: Vec<Count>,
    /// The time on each screen, longest first.
    pub screens: Vec<Time>,
    /// Each habit counted, in the rules' order.
    pub hints: Vec<Hinted>,
}

/// A group of NEVER USED; its JSON is `[group, [actions]]`, as scripts already read it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unused {
    /// The group's heading.
    pub group: String,
    /// Its actions never used.
    pub actions: Vec<String>,
}

/// An action and how often it ran; its JSON is `[action, count]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Count {
    /// The action's name.
    pub action: String,
    /// How often it ran.
    pub count: u64,
}

/// A screen and the time it was read; its JSON is `[screen, seconds]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Time {
    /// The screen's name.
    pub screen: String,
    /// Seconds read.
    pub seconds: u64,
}

/// A habit, how often it showed and what to try instead; its JSON is `[hint, count, text]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hinted {
    /// The habit's name.
    pub hint: String,
    /// How often it showed.
    pub count: u64,
    /// What to try instead.
    pub text: String,
}

impl Serialize for Unused {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (&self.group, &self.actions).serialize(serializer)
    }
}

impl Serialize for Count {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (&self.action, self.count).serialize(serializer)
    }
}

impl Serialize for Time {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (&self.screen, self.seconds).serialize(serializer)
    }
}

impl Serialize for Hinted {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (&self.hint, self.count, &self.text).serialize(serializer)
    }
}

impl Rules {
    /// Sums the days within `window` of `today`, and sorts every action the rules list into the sections.
    pub fn report(&self, days: &[Day], today: NaiveDate, window: Window) -> Report {
        let kept: Vec<&Day> = days.iter().filter(|day| window.holds(day.date, today)).collect();
        let (counted, old): (Vec<&Day>, Vec<&Day>) = kept.iter().partition(|day| day.version >= self.counting);
        let actions = summed(&kept, |day| &day.counts.actions);
        let count = |name: &str| actions.get(name).copied().unwrap_or(0);
        Report {
            days: kept.len(),
            old_days: old.len(),
            never: self.unused(self.actions.iter().copied().filter(|name| count(name) == 0)),
            rarely: self
                .actions
                .iter()
                .filter(|name| (1..=RARELY).contains(&count(name)))
                .map(|name| Count { action: (*name).to_owned(), count: count(name) })
                .collect(),
            most: most(&actions),
            screens: longest(summed(&counted, |day| &day.counts.screens)),
            hints: self.hinted(&summed(&counted, |day| &day.counts.hints)),
        }
    }

    /// `names` by group, groups in the rules' order, `other` last for any name no group lists.
    fn unused<'a>(&self, names: impl Iterator<Item = &'a str>) -> Vec<Unused> {
        let names: Vec<&str> = names.collect();
        let is_grouped = |name: &str| self.groups.iter().any(|group| group.actions.contains(&name));
        let other = Unused {
            group: "other".to_owned(),
            actions: names.iter().filter(|name| !is_grouped(name)).map(|name| (*name).to_owned()).collect(),
        };
        self.groups
            .iter()
            .map(|group| Unused {
                group: group.title.to_owned(),
                actions: group.actions.iter().filter(|action| names.contains(action)).map(|action| (*action).to_owned()).collect(),
            })
            .chain([other])
            .filter(|unused| !unused.actions.is_empty())
            .collect()
    }

    fn hinted(&self, hints: &BTreeMap<String, u64>) -> Vec<Hinted> {
        let hinted = |hint: &super::Hint| {
            hints.get(hint.name).map(|count| Hinted { hint: hint.name.to_owned(), count: *count, text: hint.text.to_owned() })
        };
        self.hints.iter().filter_map(hinted).collect()
    }
}

fn summed(days: &[&Day], counts: impl Fn(&Day) -> &BTreeMap<String, u64>) -> BTreeMap<String, u64> {
    let mut sum = BTreeMap::new();
    for day in days {
        add(&mut sum, counts(day));
    }
    sum
}

fn most(actions: &BTreeMap<String, u64>) -> Vec<Count> {
    let mut most: Vec<Count> = actions.iter().map(|(action, count)| Count { action: action.clone(), count: *count }).collect();
    most.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.action.cmp(&b.action)));
    most.truncate(MOST);
    most
}

fn longest(screens: BTreeMap<String, u64>) -> Vec<Time> {
    let mut times: Vec<Time> = screens.into_iter().map(|(screen, seconds)| Time { screen, seconds }).collect();
    times.sort_by_key(|time| std::cmp::Reverse(time.seconds));
    times
}

/// `1h 12m`, `4m`, `35s`.
pub fn duration(seconds: u64) -> String {
    match (seconds / 3600, (seconds % 3600) / 60) {
        (0, 0) => format!("{seconds}s"),
        (0, minutes) => format!("{minutes}m"),
        (hours, minutes) => format!("{hours}h {minutes}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::Counts;
    use super::super::fixtures::{date, rules};
    use super::*;

    fn count(action: &str, count: u64) -> Count {
        Count { action: action.to_owned(), count }
    }

    fn fixture() -> Vec<Day> {
        rules().parse(
            "{\"date\":\"2026-08-01\",\"actions\":{\"merge\":9}}\n\
             {\"date\":\"2026-09-20\",\"version\":2,\"actions\":{\"next_thread\":40,\"zen\":2,\"move_down\":120},\"screens\":{\"diff\":3600,\"queue\":120},\"hints\":{\"long_walk\":3}}\n\
             {\"date\":\"2026-09-24\",\"actions\":{\"next_thread\":2,\"react\":1}}\n",
        )
    }

    fn last_30_days() -> Report {
        rules().report(&fixture(), date("2026-09-24"), Window::Days(30))
    }

    #[test]
    fn a_window_reads_days_weeks_and_all() {
        assert_eq!("30d".parse(), Ok(Window::Days(30)));
        assert_eq!("2w".parse(), Ok(Window::Days(14)));
        assert_eq!("all".parse(), Ok(Window::All));
    }

    #[test]
    fn a_window_without_its_unit_or_its_number_is_refused() {
        for text in ["30", "d", "30é", ""] {
            assert_eq!(text.parse::<Window>(), Err(BadWindow), "{text}");
        }
    }

    #[test]
    fn a_report_leaves_out_the_days_before_its_window() {
        assert_eq!(last_30_days().days, 2);
        assert_eq!(rules().report(&fixture(), date("2026-09-24"), Window::All).days, 3);
    }

    #[test]
    fn most_used_lists_the_largest_counts_first() {
        assert_eq!(last_30_days().most[..2], [count("move_down", 120), count("next_thread", 42)]);
    }

    #[test]
    fn rarely_used_lists_the_actions_counted_twice_or_less_in_the_rules_order() {
        assert_eq!(last_30_days().rarely, [count("zen", 2), count("react", 1)]);
    }

    #[test]
    fn never_used_groups_the_listed_actions_the_window_never_counted() {
        let never = last_30_days().never;
        assert_eq!(
            never,
            [
                Unused { group: "view".to_owned(), actions: vec!["side_by_side".to_owned()] },
                Unused { group: "other".to_owned(), actions: vec!["merge".to_owned()] }
            ]
        );
    }

    #[test]
    fn time_and_hints_are_summed_longest_first() {
        let report = last_30_days();
        assert_eq!(report.screens[0], Time { screen: "diff".to_owned(), seconds: 3600 });
        assert_eq!(report.hints, [Hinted { hint: "long_walk".to_owned(), count: 3, text: rules().hints[0].text.to_owned() }]);
    }

    #[test]
    fn a_day_counted_the_old_way_keeps_its_actions_and_leaves_out_its_time_and_hints() {
        let old = "{\"date\":\"2026-09-23\",\"actions\":{\"zen\":5},\"screens\":{\"diff\":36000},\"hints\":{\"long_walk\":40}}\n";
        let minute = Counts { screens: BTreeMap::from([("diff".to_owned(), 60)]), ..Counts::default() };
        let text = rules().merge(&rules().merge(old, date("2026-09-24"), &minute).unwrap(), date("2026-09-23"), &minute).unwrap();
        let report = rules().report(&rules().parse(&text), date("2026-09-24"), Window::All);
        assert_eq!((report.days, report.old_days), (2, 1));
        assert_eq!(report.most[0], count("zen", 5));
        assert_eq!(report.screens, [Time { screen: "diff".to_owned(), seconds: 60 }], "only the new day's minute");
        assert_eq!(report.hints, []);
    }

    #[test]
    fn the_report_json_keeps_each_entry_as_an_array() {
        let report = Report {
            never: vec![Unused { group: "move".to_owned(), actions: vec!["zen".to_owned()] }],
            rarely: vec![count("zen", 1)],
            ..Report::default()
        };
        let json = serde_json::to_string(&report).unwrap();
        assert_eq!(
            json,
            "{\"days\":0,\"old_days\":0,\"never\":[[\"move\",[\"zen\"]]],\"rarely\":[[\"zen\",1]],\"most\":[],\"screens\":[],\"hints\":[]}"
        );
    }

    #[test]
    fn durations_read_short() {
        assert_eq!(duration(35), "35s");
        assert_eq!(duration(240), "4m");
        assert_eq!(duration(4320), "1h 12m");
    }
}
