//! Reward math: deriving earned effort from session history.
//!
//! Effort is always derived here from completed focus sessions — never read from a
//! counter stored on the task (see `SCOPE.md`).

use crate::domain::session::{PomodoroSession, SessionStatus, TimerPhase};

/// Counts the completed focus sessions in `sessions` — the effort that earns reward.
///
/// Break phases and abandoned sessions do not count. The `usize` count is narrowed to
/// `u32`, saturating at [`u32::MAX`] for an implausibly long history rather than panicking.
#[must_use]
pub fn completed_focus_pomos(sessions: &[PomodoroSession]) -> u32 {
    let count = sessions
        .iter()
        .filter(|session| {
            session.phase() == TimerPhase::Focus && session.status() == SessionStatus::Completed
        })
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// Converts earned pomos into leisure minutes at the configured rate.
///
/// Saturates at [`u32::MAX`] rather than overflowing.
#[must_use]
pub fn leisure_minutes(pomos: u32, minutes_per_pomo: u32) -> u32 {
    pomos.saturating_mul(minutes_per_pomo)
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::domain::ids::{PomodoroSessionId, TaskId};

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(0, 0).expect("epoch is a valid timestamp")
    }

    fn finished(phase: TimerPhase, completed: bool) -> PomodoroSession {
        let mut session =
            PomodoroSession::new(PomodoroSessionId::new(1), TaskId::new(1), phase, 1500, ts());
        if completed {
            session.complete(ts());
        } else {
            session.abandon(ts());
        }
        session
    }

    #[test]
    fn empty_history_earns_nothing() {
        assert_eq!(completed_focus_pomos(&[]), 0);
    }

    #[test]
    fn counts_only_completed_focus_sessions() {
        let sessions = [
            finished(TimerPhase::Focus, true),
            finished(TimerPhase::Focus, true),
            finished(TimerPhase::Focus, false), // abandoned — not counted
            finished(TimerPhase::ShortBreak, true), // break — not counted
            finished(TimerPhase::LongBreak, true), // break — not counted
        ];
        assert_eq!(completed_focus_pomos(&sessions), 2);
    }

    #[test]
    fn leisure_minutes_multiplies_pomos_by_rate() {
        assert_eq!(leisure_minutes(3, 5), 15);
        assert_eq!(leisure_minutes(0, 5), 0);
        assert_eq!(leisure_minutes(16, 5), 80);
    }
}
