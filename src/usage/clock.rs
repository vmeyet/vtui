//! Screen time: a screen counts while the user reads it, and stops a minute after the last input or while the terminal
//! is in the background.
use std::time::{Duration, Instant};

/// Reading goes on this long after the last key, click or wheel; past it the screen is left alone, not read.
const IDLE_AFTER: Duration = Duration::from_secs(60);

/// Up to when screen time was counted, the last input, and whether the terminal is in front.
/// The app reads the clock and hands `now` in, so its own state stays free of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clock {
    counted_to: Instant,
    last_input: Instant,
    is_in_front: bool,
}

impl Clock {
    /// A clock counting from `now`, the terminal in front.
    pub fn starting(now: Instant) -> Self {
        Self { counted_to: now, last_input: now, is_in_front: true }
    }

    /// The time read since the clock last counted, for the screen on show; the clock then counts from `now`.
    pub fn count(&mut self, now: Instant) -> Duration {
        let read_until = now.min(self.last_input + IDLE_AFTER);
        let read = if self.is_in_front { read_until.saturating_duration_since(self.counted_to) } else { Duration::ZERO };
        self.counted_to = now;
        read
    }

    /// A key, click or wheel at `now`: the time read before it, for the screen it landed on; reading goes on from now.
    pub fn input(&mut self, now: Instant) -> Duration {
        let read = self.count(now);
        self.last_input = now;
        read
    }

    /// The terminal came to the front at `now`: the clock runs again from now.
    pub fn to_front(&mut self, now: Instant) -> Duration {
        let read = self.input(now);
        self.is_in_front = true;
        read
    }

    /// The terminal went to the background at `now`: the time read before it, and the clock stops until it is back.
    pub fn to_back(&mut self, now: Instant) -> Duration {
        let read = self.input(now);
        self.is_in_front = false;
        read
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seconds(count: u64) -> Duration {
        Duration::from_secs(count)
    }

    #[test]
    fn a_screen_counts_the_time_between_two_counts() {
        let start = Instant::now();
        let mut clock = Clock::starting(start);
        assert_eq!(clock.count(start + seconds(10)), seconds(10));
        assert_eq!(clock.count(start + seconds(15)), seconds(5));
    }

    #[test]
    fn time_stops_a_minute_after_the_last_input() {
        let start = Instant::now();
        let mut clock = Clock::starting(start);
        assert_eq!(clock.count(start + seconds(300)), seconds(60));
        assert_eq!(clock.input(start + seconds(400)), Duration::ZERO, "the idle minutes before the key are not read");
        assert_eq!(clock.count(start + seconds(410)), seconds(10));
    }

    #[test]
    fn time_stops_while_the_terminal_is_in_the_background() {
        let start = Instant::now();
        let mut clock = Clock::starting(start);
        assert_eq!(clock.to_back(start + seconds(5)), seconds(5));
        assert_eq!(clock.to_front(start + seconds(50)), Duration::ZERO);
        assert_eq!(clock.count(start + seconds(53)), seconds(3));
    }
}
