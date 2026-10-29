//! Double and triple clicks: the terminal reports each press alone, so the app counts them from the time it reads.
use ratatui::layout::Position;
use std::time::{Duration, Instant};

/// How soon a press must follow the last one, on the same cell, to count with it; about what desktops use.
const WITHIN: Duration = Duration::from_millis(400);

/// How many presses in a row a click is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Click {
    /// A press alone: it places the cursor.
    Single,
    /// The second press: it takes a word.
    Double,
    /// The third press: it takes a line.
    Triple,
}

/// The last press of the left button, so the next one knows whether it follows it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Clicks {
    last: Option<Press>,
}

#[derive(Clone, Copy, Debug)]
struct Press {
    at: Position,
    when: Instant,
    click: Click,
}

impl Clicks {
    /// Counts a press at `at` read at `now`: one on the same cell within 400 ms of the last makes it a double, then a
    /// triple click, and the one after a triple starts over.
    pub fn press(&mut self, at: Position, now: Instant) -> Click {
        let follows = self.last.filter(|last| last.at == at && now.saturating_duration_since(last.when) <= WITHIN);
        let click = match follows.map(|last| last.click) {
            Some(Click::Single) => Click::Double,
            Some(Click::Double) => Click::Triple,
            Some(Click::Triple) | None => Click::Single,
        };
        self.last = Some(Press { at, when: now, click });
        click
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CELL: Position = Position { x: 4, y: 2 };

    fn presses(clicks: &mut Clicks, at: Position, start: Instant, gaps_ms: &[u64]) -> Vec<Click> {
        let mut when = start;
        gaps_ms
            .iter()
            .map(|&gap| {
                when += Duration::from_millis(gap);
                clicks.press(at, when)
            })
            .collect()
    }

    #[test]
    fn quick_presses_on_one_cell_count_up_to_three_then_start_over() {
        let clicks = presses(&mut Clicks::default(), CELL, Instant::now(), &[0, 100, 100, 100]);
        assert_eq!(clicks, [Click::Single, Click::Double, Click::Triple, Click::Single]);
    }

    #[test]
    fn a_slow_press_is_a_single_click() {
        let clicks = presses(&mut Clicks::default(), CELL, Instant::now(), &[0, 500]);
        assert_eq!(clicks, [Click::Single, Click::Single]);
    }

    #[test]
    fn a_press_on_another_cell_is_a_single_click() {
        let mut clicks = Clicks::default();
        let now = Instant::now();
        clicks.press(CELL, now);
        assert_eq!(clicks.press(Position { x: 5, y: 2 }, now), Click::Single);
    }
}
