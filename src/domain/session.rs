//! Pomodoro sessions: a single timer run bound to a task.

use chrono::{DateTime, Utc};

use crate::domain::ids::{PomodoroSessionId, TaskId};

/// Which phase of the Pomodoro cycle a session represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimerPhase {
    /// A focus interval — the only phase that counts toward task reward.
    Focus,
    /// A short break between focus intervals.
    ShortBreak,
    /// A longer break after a configured number of focus intervals.
    LongBreak,
}

impl TimerPhase {
    /// Returns `true` if effort in this phase counts toward task reward.
    #[must_use]
    pub fn is_effort(self) -> bool {
        matches!(self, Self::Focus)
    }
}

/// Lifecycle of a single [`PomodoroSession`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionStatus {
    /// The timer is currently running.
    Running,
    /// The phase ran to completion (its full configured duration elapsed).
    Completed,
    /// The phase was abandoned before completing; its partial time is discarded.
    Abandoned,
}

/// A single timer session bound to a task.
///
/// "Banked" status is tracked via the ledger, not on the session, so a completed focus
/// session cannot be double-counted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PomodoroSession {
    id: PomodoroSessionId,
    task_id: TaskId,
    phase: TimerPhase,
    status: SessionStatus,
    configured_duration_seconds: u32,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

impl PomodoroSession {
    /// Starts a new session in the [`SessionStatus::Running`] state.
    #[must_use]
    pub fn new(
        id: PomodoroSessionId,
        task_id: TaskId,
        phase: TimerPhase,
        configured_duration_seconds: u32,
        started_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            task_id,
            phase,
            status: SessionStatus::Running,
            configured_duration_seconds,
            started_at,
            finished_at: None,
        }
    }

    /// Reconstructs a session from persisted fields.
    ///
    /// Intended for the storage layer when hydrating a row.
    #[must_use]
    pub fn from_fields(
        id: PomodoroSessionId,
        task_id: TaskId,
        phase: TimerPhase,
        status: SessionStatus,
        configured_duration_seconds: u32,
        started_at: DateTime<Utc>,
        finished_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id,
            task_id,
            phase,
            status,
            configured_duration_seconds,
            started_at,
            finished_at,
        }
    }

    /// Marks the session completed at `finished_at` (the full duration elapsed).
    pub fn complete(&mut self, finished_at: DateTime<Utc>) {
        self.status = SessionStatus::Completed;
        self.finished_at = Some(finished_at);
    }

    /// Marks the session abandoned at `finished_at`; its partial time is discarded.
    pub fn abandon(&mut self, finished_at: DateTime<Utc>) {
        self.status = SessionStatus::Abandoned;
        self.finished_at = Some(finished_at);
    }

    /// The session's identifier.
    #[must_use]
    pub fn id(&self) -> PomodoroSessionId {
        self.id
    }

    /// The task this session belongs to.
    #[must_use]
    pub fn task_id(&self) -> TaskId {
        self.task_id
    }

    /// The Pomodoro phase this session represents.
    #[must_use]
    pub fn phase(&self) -> TimerPhase {
        self.phase
    }

    /// The session's current lifecycle status.
    #[must_use]
    pub fn status(&self) -> SessionStatus {
        self.status
    }

    /// The phase's configured duration, in seconds.
    #[must_use]
    pub fn configured_duration_seconds(&self) -> u32 {
        self.configured_duration_seconds
    }

    /// When the session started.
    #[must_use]
    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    /// When the session finished (completed or abandoned), if it has.
    #[must_use]
    pub fn finished_at(&self) -> Option<DateTime<Utc>> {
        self.finished_at
    }

    /// Seconds elapsed since the session started, as of `now`.
    #[must_use]
    pub fn elapsed_seconds(&self, now: DateTime<Utc>) -> i64 {
        (now - self.started_at).num_seconds()
    }

    /// Seconds left until the configured duration elapses (negative once past it).
    ///
    /// This is derived purely from `started_at` and the configured duration, so a running
    /// timer is restored correctly after an app restart.
    #[must_use]
    pub fn remaining_seconds(&self, now: DateTime<Utc>) -> i64 {
        i64::from(self.configured_duration_seconds) - self.elapsed_seconds(now)
    }

    /// Whether the configured duration has fully elapsed as of `now`.
    #[must_use]
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.remaining_seconds(now) <= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(0, 0).expect("epoch is a valid timestamp")
    }

    fn running_focus() -> PomodoroSession {
        PomodoroSession::new(
            PomodoroSessionId::new(1),
            TaskId::new(1),
            TimerPhase::Focus,
            1500,
            ts(),
        )
    }

    #[test]
    fn only_focus_counts_as_effort() {
        assert!(TimerPhase::Focus.is_effort());
        assert!(!TimerPhase::ShortBreak.is_effort());
        assert!(!TimerPhase::LongBreak.is_effort());
    }

    #[test]
    fn new_session_starts_running_and_unfinished() {
        let session = running_focus();
        assert_eq!(session.status(), SessionStatus::Running);
        assert_eq!(session.finished_at(), None);
    }

    #[test]
    fn complete_sets_status_and_finished_at() {
        let mut session = running_focus();
        session.complete(ts());
        assert_eq!(session.status(), SessionStatus::Completed);
        assert_eq!(session.finished_at(), Some(ts()));
    }

    #[test]
    fn abandon_sets_status_and_finished_at() {
        let mut session = running_focus();
        session.abandon(ts());
        assert_eq!(session.status(), SessionStatus::Abandoned);
        assert_eq!(session.finished_at(), Some(ts()));
    }

    #[test]
    fn remaining_and_expiry_track_elapsed_time() {
        let start = DateTime::from_timestamp(1000, 0).expect("valid timestamp");
        let session = PomodoroSession::new(
            PomodoroSessionId::new(1),
            TaskId::new(1),
            TimerPhase::Focus,
            60,
            start,
        );

        assert_eq!(session.remaining_seconds(start), 60);
        assert!(!session.is_expired(start));

        let midway = DateTime::from_timestamp(1030, 0).expect("valid timestamp");
        assert_eq!(session.remaining_seconds(midway), 30);
        assert!(!session.is_expired(midway));

        let at_end = DateTime::from_timestamp(1060, 0).expect("valid timestamp");
        assert_eq!(session.remaining_seconds(at_end), 0);
        assert!(session.is_expired(at_end));

        let past = DateTime::from_timestamp(1100, 0).expect("valid timestamp");
        assert!(session.remaining_seconds(past) < 0);
        assert!(session.is_expired(past));
    }
}
