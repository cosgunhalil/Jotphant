//! Tasks and their lifecycle state machine.

use chrono::{DateTime, Utc};

use crate::domain::ids::TaskId;

/// Lifecycle state of a [`Task`].
///
/// Valid transitions (see `SCOPE.md`):
/// `Todo → InProgress`, `InProgress ⇄ Paused`, and `{InProgress, Paused} → {Done,
/// Cancelled}`. `Done` and `Cancelled` are terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    /// Created but not yet started.
    Todo,
    /// Active and owning the focus timer. At most one task is in this state.
    InProgress,
    /// Started but temporarily suspended; retains its completed pomos.
    Paused,
    /// Completed; terminal.
    Done,
    /// Abandoned; terminal.
    Cancelled,
}

impl TaskStatus {
    /// Returns `true` if no further transitions are allowed.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::Cancelled)
    }

    /// Returns `true` if this is the single active (timer-owning) state.
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(self, Self::InProgress)
    }

    /// Returns `true` if a direct transition to `target` is permitted.
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            (Self::Todo, Self::InProgress)
                | (Self::InProgress, Self::Paused | Self::Done | Self::Cancelled)
                | (Self::Paused, Self::InProgress | Self::Done | Self::Cancelled)
        )
    }

    /// Validates a transition, returning the target status on success.
    ///
    /// # Errors
    /// Returns [`InvalidTransition`] if the transition is not permitted.
    pub fn transition_to(self, target: Self) -> Result<Self, InvalidTransition> {
        if self.can_transition_to(target) {
            Ok(target)
        } else {
            Err(InvalidTransition::new(self, target))
        }
    }
}

/// Error returned when an invalid [`TaskStatus`] transition is attempted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("invalid task transition from {from:?} to {to:?}")]
pub struct InvalidTransition {
    from: TaskStatus,
    to: TaskStatus,
}

impl InvalidTransition {
    fn new(from: TaskStatus, to: TaskStatus) -> Self {
        Self { from, to }
    }

    /// The status the task was in.
    #[must_use]
    pub fn from(self) -> TaskStatus {
        self.from
    }

    /// The rejected target status.
    #[must_use]
    pub fn to(self) -> TaskStatus {
        self.to
    }
}

/// A unit of work whose focused effort is measured in pomodoros.
///
/// Completed effort is never stored on the task; it is derived from session history
/// (see [`crate::domain::reward`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    id: TaskId,
    title: String,
    description: String,
    status: TaskStatus,
    estimated_pomos: u32,
    linked_from: Option<TaskId>,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

impl Task {
    /// Creates a new task in the [`TaskStatus::Todo`] state.
    #[must_use]
    pub fn new(
        id: TaskId,
        title: String,
        estimated_pomos: u32,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            title,
            description: String::new(),
            status: TaskStatus::Todo,
            estimated_pomos,
            linked_from: None,
            created_at,
            completed_at: None,
        }
    }

    /// Reconstructs a task from persisted fields.
    ///
    /// Intended for the storage layer when hydrating a row; it trusts the given values
    /// rather than enforcing the `Todo` starting state that [`Task::new`] imposes.
    // Every persisted column is required to rebuild the row faithfully.
    #[expect(clippy::too_many_arguments, reason = "hydrates a full persisted row")]
    #[must_use]
    pub fn from_fields(
        id: TaskId,
        title: String,
        description: String,
        status: TaskStatus,
        estimated_pomos: u32,
        linked_from: Option<TaskId>,
        created_at: DateTime<Utc>,
        completed_at: Option<DateTime<Utc>>,
    ) -> Self {
        Self {
            id,
            title,
            description,
            status,
            estimated_pomos,
            linked_from,
            created_at,
            completed_at,
        }
    }

    /// Attempts to move the task to `target`.
    ///
    /// On a successful transition to [`TaskStatus::Done`], `now` is recorded as the
    /// completion time. On failure the task is left unchanged.
    ///
    /// # Errors
    /// Returns [`InvalidTransition`] if the transition is not permitted.
    pub fn apply_transition(
        &mut self,
        target: TaskStatus,
        now: DateTime<Utc>,
    ) -> Result<(), InvalidTransition> {
        let next = self.status.transition_to(target)?;
        self.status = next;
        if next == TaskStatus::Done {
            self.completed_at = Some(now);
        }
        Ok(())
    }

    /// The task's identifier.
    #[must_use]
    pub fn id(&self) -> TaskId {
        self.id
    }

    /// The task's title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The task's free-form description (may be empty).
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Replaces the task's description.
    pub fn set_description(&mut self, description: String) {
        self.description = description;
    }

    /// The task's current lifecycle status.
    #[must_use]
    pub fn status(&self) -> TaskStatus {
        self.status
    }

    /// The user's pomodoro estimate for the task.
    #[must_use]
    pub fn estimated_pomos(&self) -> u32 {
        self.estimated_pomos
    }

    /// The task this one was created as a follow-up to, if any.
    #[must_use]
    pub fn linked_from(&self) -> Option<TaskId> {
        self.linked_from
    }

    /// When the task was created.
    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// When the task was completed, if it has been.
    #[must_use]
    pub fn completed_at(&self) -> Option<DateTime<Utc>> {
        self.completed_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(0, 0).expect("epoch is a valid timestamp")
    }

    #[test]
    fn todo_can_start_but_not_finish_directly() {
        assert!(TaskStatus::Todo.can_transition_to(TaskStatus::InProgress));
        assert!(!TaskStatus::Todo.can_transition_to(TaskStatus::Done));
        assert!(!TaskStatus::Todo.can_transition_to(TaskStatus::Cancelled));
    }

    #[test]
    fn in_progress_can_pause_complete_or_cancel() {
        for target in [TaskStatus::Paused, TaskStatus::Done, TaskStatus::Cancelled] {
            assert!(TaskStatus::InProgress.can_transition_to(target));
        }
    }

    #[test]
    fn paused_can_resume_complete_or_cancel() {
        for target in [TaskStatus::InProgress, TaskStatus::Done, TaskStatus::Cancelled] {
            assert!(TaskStatus::Paused.can_transition_to(target));
        }
    }

    #[test]
    fn terminal_states_reject_every_transition() {
        let all = [
            TaskStatus::Todo,
            TaskStatus::InProgress,
            TaskStatus::Paused,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ];
        for from in [TaskStatus::Done, TaskStatus::Cancelled] {
            assert!(from.is_terminal());
            for to in all {
                assert!(!from.can_transition_to(to));
            }
        }
    }

    #[test]
    fn transition_to_reports_from_and_to_on_error() {
        let err = TaskStatus::Todo
            .transition_to(TaskStatus::Done)
            .expect_err("todo cannot go straight to done");
        assert_eq!(err.from(), TaskStatus::Todo);
        assert_eq!(err.to(), TaskStatus::Done);
    }

    #[test]
    fn apply_transition_to_done_records_completed_at() {
        let mut task = Task::new(TaskId::new(1), "write tests".to_owned(), 4, ts());
        task.apply_transition(TaskStatus::InProgress, ts())
            .expect("todo can start");
        assert_eq!(task.completed_at(), None);

        task.apply_transition(TaskStatus::Done, ts())
            .expect("in-progress can complete");
        assert_eq!(task.status(), TaskStatus::Done);
        assert_eq!(task.completed_at(), Some(ts()));
    }

    #[test]
    fn invalid_apply_transition_leaves_task_unchanged() {
        let mut task = Task::new(TaskId::new(1), "x".to_owned(), 1, ts());
        let result = task.apply_transition(TaskStatus::Done, ts());
        assert!(result.is_err());
        assert_eq!(task.status(), TaskStatus::Todo);
    }
}
