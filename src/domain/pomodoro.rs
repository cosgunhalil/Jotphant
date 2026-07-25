//! Pomodoro cycle configuration and the pure phase-sequencing rules.
//!
//! The rules here decide *what* comes next in the Focus → Break → Focus cycle; the app
//! layer applies them by creating sessions. Loading these values from a config file is
//! the storage/app layer's job (M2); the domain only holds and reasons about them.

use crate::domain::session::TimerPhase;

/// Timing and auto-start policy for the Pomodoro cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PomodoroConfig {
    focus_seconds: u32,
    short_break_seconds: u32,
    long_break_seconds: u32,
    long_break_after: u32,
    auto_start_break: bool,
    auto_start_focus: bool,
}

impl PomodoroConfig {
    /// Builds a config. `long_break_after` is clamped to at least 1 so the cycle math
    /// never divides by zero.
    #[must_use]
    pub fn new(
        focus_seconds: u32,
        short_break_seconds: u32,
        long_break_seconds: u32,
        long_break_after: u32,
        auto_start_break: bool,
        auto_start_focus: bool,
    ) -> Self {
        Self {
            focus_seconds,
            short_break_seconds,
            long_break_seconds,
            long_break_after: long_break_after.max(1),
            auto_start_break,
            auto_start_focus,
        }
    }

    /// How many completed focus pomos precede each long break.
    #[must_use]
    pub fn long_break_after(&self) -> u32 {
        self.long_break_after
    }

    /// The configured duration, in seconds, of a given phase.
    #[must_use]
    pub fn duration_seconds(&self, phase: TimerPhase) -> u32 {
        match phase {
            TimerPhase::Focus => self.focus_seconds,
            TimerPhase::ShortBreak => self.short_break_seconds,
            TimerPhase::LongBreak => self.long_break_seconds,
        }
    }

    /// The phase that should follow completing `completed`.
    ///
    /// `completed_focus_count` is the task's total completed focus pomos (including the
    /// one just finished); every `long_break_after` of them a long break is due.
    #[must_use]
    pub fn next_phase(&self, completed: TimerPhase, completed_focus_count: u32) -> TimerPhase {
        match completed {
            TimerPhase::Focus => {
                if completed_focus_count.is_multiple_of(self.long_break_after) {
                    TimerPhase::LongBreak
                } else {
                    TimerPhase::ShortBreak
                }
            }
            TimerPhase::ShortBreak | TimerPhase::LongBreak => TimerPhase::Focus,
        }
    }

    /// Whether starting `phase` automatically (vs. awaiting confirmation) is enabled.
    #[must_use]
    pub fn should_auto_start(&self, phase: TimerPhase) -> bool {
        match phase {
            TimerPhase::Focus => self.auto_start_focus,
            TimerPhase::ShortBreak | TimerPhase::LongBreak => self.auto_start_break,
        }
    }
}

impl Default for PomodoroConfig {
    /// 25-minute focus, 5-minute short break, 15-minute long break every 4 focus pomos,
    /// with breaks and the following focus starting automatically.
    fn default() -> Self {
        Self::new(25 * 60, 5 * 60, 15 * 60, 4, true, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_normally_leads_to_a_short_break() {
        let config = PomodoroConfig::default();
        assert_eq!(
            config.next_phase(TimerPhase::Focus, 1),
            TimerPhase::ShortBreak
        );
        assert_eq!(
            config.next_phase(TimerPhase::Focus, 3),
            TimerPhase::ShortBreak
        );
    }

    #[test]
    fn every_nth_focus_leads_to_a_long_break() {
        let config = PomodoroConfig::default(); // long break after 4
        assert_eq!(
            config.next_phase(TimerPhase::Focus, 4),
            TimerPhase::LongBreak
        );
        assert_eq!(
            config.next_phase(TimerPhase::Focus, 8),
            TimerPhase::LongBreak
        );
    }

    #[test]
    fn breaks_lead_back_to_focus() {
        let config = PomodoroConfig::default();
        assert_eq!(
            config.next_phase(TimerPhase::ShortBreak, 3),
            TimerPhase::Focus
        );
        assert_eq!(
            config.next_phase(TimerPhase::LongBreak, 4),
            TimerPhase::Focus
        );
    }

    #[test]
    fn duration_matches_the_phase() {
        let config = PomodoroConfig::default();
        assert_eq!(config.duration_seconds(TimerPhase::Focus), 1500);
        assert_eq!(config.duration_seconds(TimerPhase::ShortBreak), 300);
        assert_eq!(config.duration_seconds(TimerPhase::LongBreak), 900);
    }

    #[test]
    fn auto_start_flags_are_reported_per_phase() {
        let default = PomodoroConfig::default();
        assert!(default.should_auto_start(TimerPhase::Focus));
        assert!(default.should_auto_start(TimerPhase::ShortBreak));

        let manual = PomodoroConfig::new(1, 1, 1, 4, false, false);
        assert!(!manual.should_auto_start(TimerPhase::Focus));
        assert!(!manual.should_auto_start(TimerPhase::ShortBreak));
    }

    #[test]
    fn long_break_after_is_clamped_to_at_least_one() {
        let config = PomodoroConfig::new(1, 1, 1, 0, true, true);
        // With the clamp, every focus is "the nth", so a long break is always due.
        assert_eq!(
            config.next_phase(TimerPhase::Focus, 1),
            TimerPhase::LongBreak
        );
        assert_eq!(
            config.next_phase(TimerPhase::Focus, 2),
            TimerPhase::LongBreak
        );
    }
}
