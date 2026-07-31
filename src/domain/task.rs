//! Tasks and their lifecycle state machine.

use chrono::{DateTime, NaiveDate, Utc};

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
                | (
                    Self::InProgress,
                    Self::Paused | Self::Done | Self::Cancelled
                )
                | (
                    Self::Paused,
                    Self::InProgress | Self::Done | Self::Cancelled
                )
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

/// A three-step rating used for a task's expected effort and effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rating {
    /// Low.
    Low,
    /// Medium.
    Mid,
    /// High.
    High,
}

impl Rating {
    /// The rating as a number (0, 1, 2) for score arithmetic.
    #[must_use]
    pub fn level(self) -> i8 {
        match self {
            Self::Low => 0,
            Self::Mid => 1,
            Self::High => 2,
        }
    }
}

/// Error returned when a task's due date would fall before its start date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the due date is before the start date")]
pub struct InvalidSchedule;

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
    effort: Option<Rating>,
    effect: Option<Rating>,
    start_date: Option<NaiveDate>,
    due_date: Option<NaiveDate>,
    linked_from: Option<TaskId>,
    created_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

impl Task {
    /// Creates a new task in the [`TaskStatus::Todo`] state.
    #[must_use]
    pub fn new(id: TaskId, title: String, estimated_pomos: u32, created_at: DateTime<Utc>) -> Self {
        Self {
            id,
            title,
            description: String::new(),
            status: TaskStatus::Todo,
            estimated_pomos,
            effort: None,
            effect: None,
            start_date: None,
            due_date: None,
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
        effort: Option<Rating>,
        effect: Option<Rating>,
        start_date: Option<NaiveDate>,
        due_date: Option<NaiveDate>,
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
            effort,
            effect,
            start_date,
            due_date,
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

    /// The expected effort rating, if the user has set one.
    #[must_use]
    pub fn effort(&self) -> Option<Rating> {
        self.effort
    }

    /// The expected effect (impact) rating, if the user has set one.
    #[must_use]
    pub fn effect(&self) -> Option<Rating> {
        self.effect
    }

    /// Sets or clears the effort and effect ratings.
    pub fn set_ratings(&mut self, effort: Option<Rating>, effect: Option<Rating>) {
        self.effort = effort;
        self.effect = effect;
    }

    /// The task's value-for-effort score, when **both** ratings are set:
    /// `effect − effort`, ranging from −2 (high effort, low effect — a money pit)
    /// through 0 (balanced) to +2 (low effort, high effect — a quick win).
    #[must_use]
    pub fn value_score(&self) -> Option<i8> {
        match (self.effect, self.effort) {
            (Some(effect), Some(effort)) => Some(effect.level() - effort.level()),
            _ => None,
        }
    }

    /// The planned start date, if scheduled.
    #[must_use]
    pub fn start_date(&self) -> Option<NaiveDate> {
        self.start_date
    }

    /// The due date, if set.
    #[must_use]
    pub fn due_date(&self) -> Option<NaiveDate> {
        self.due_date
    }

    /// Sets or clears the planned start and due dates.
    ///
    /// # Errors
    /// Returns [`InvalidSchedule`] if both are set and the due date is before the
    /// start date; the task is left unchanged.
    pub fn set_schedule(
        &mut self,
        start_date: Option<NaiveDate>,
        due_date: Option<NaiveDate>,
    ) -> Result<(), InvalidSchedule> {
        if let (Some(start), Some(due)) = (start_date, due_date)
            && due < start
        {
            return Err(InvalidSchedule);
        }
        self.start_date = start_date;
        self.due_date = due_date;
        Ok(())
    }

    /// Whether the task is past its due date and still unfinished, as of `today`.
    #[must_use]
    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        match self.due_date {
            Some(due) => due < today && !self.status.is_terminal(),
            None => false,
        }
    }

    /// The date the task's timeline bar begins: the planned start date, falling back
    /// to the day the task was created.
    #[must_use]
    pub fn bar_start(&self) -> NaiveDate {
        self.start_date
            .unwrap_or_else(|| self.created_at.date_naive())
    }

    /// The task this one was created as a follow-up to, if any.
    #[must_use]
    pub fn linked_from(&self) -> Option<TaskId> {
        self.linked_from
    }

    /// Updates the pomodoro estimate.
    pub fn set_estimated_pomos(&mut self, estimated_pomos: u32) {
        self.estimated_pomos = estimated_pomos;
    }

    /// Sets the task this one follows up on.
    pub fn set_linked_from(&mut self, linked_from: Option<TaskId>) {
        self.linked_from = linked_from;
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
        for target in [
            TaskStatus::InProgress,
            TaskStatus::Done,
            TaskStatus::Cancelled,
        ] {
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
    fn value_score_requires_both_ratings() {
        let mut task = Task::new(TaskId::new(1), "rate me".to_owned(), 1, ts());
        assert_eq!(task.value_score(), None);

        task.set_ratings(Some(Rating::High), None);
        assert_eq!(task.value_score(), None);

        // Quick win: high effect for low effort.
        task.set_ratings(Some(Rating::Low), Some(Rating::High));
        assert_eq!(task.value_score(), Some(2));
        // Money pit: high effort for low effect.
        task.set_ratings(Some(Rating::High), Some(Rating::Low));
        assert_eq!(task.value_score(), Some(-2));
        // Balanced.
        task.set_ratings(Some(Rating::Mid), Some(Rating::Mid));
        assert_eq!(task.value_score(), Some(0));

        // Ratings can be cleared again.
        task.set_ratings(None, None);
        assert_eq!(task.value_score(), None);
    }

    #[test]
    fn schedule_rejects_due_before_start() {
        let mut task = Task::new(TaskId::new(1), "plan me".to_owned(), 1, ts());
        let start = NaiveDate::from_ymd_opt(2026, 8, 10).expect("valid date");
        let due = NaiveDate::from_ymd_opt(2026, 8, 3).expect("valid date");

        assert_eq!(
            task.set_schedule(Some(start), Some(due)),
            Err(InvalidSchedule)
        );
        // The failed call changed nothing.
        assert_eq!(task.start_date(), None);
        assert_eq!(task.due_date(), None);

        // A valid window (and single-sided dates) are accepted, and clearable.
        task.set_schedule(Some(due), Some(start))
            .expect("valid window");
        assert_eq!(task.start_date(), Some(due));
        assert_eq!(task.due_date(), Some(start));
        task.set_schedule(None, Some(start)).expect("due only");
        task.set_schedule(None, None).expect("cleared");
    }

    #[test]
    fn overdue_requires_a_passed_due_date_and_an_open_task() {
        let mut task = Task::new(TaskId::new(1), "deadline".to_owned(), 1, ts());
        let due = NaiveDate::from_ymd_opt(2026, 8, 3).expect("valid date");
        let before = NaiveDate::from_ymd_opt(2026, 8, 1).expect("valid date");
        let after = NaiveDate::from_ymd_opt(2026, 8, 5).expect("valid date");

        // No due date -> never overdue.
        assert!(!task.is_overdue(after));

        task.set_schedule(None, Some(due)).expect("set due");
        assert!(!task.is_overdue(before));
        assert!(!task.is_overdue(due)); // due today is not yet overdue
        assert!(task.is_overdue(after));

        // Finished tasks are never overdue.
        task.apply_transition(TaskStatus::InProgress, ts())
            .expect("start");
        task.apply_transition(TaskStatus::Done, ts()).expect("done");
        assert!(!task.is_overdue(after));
    }

    #[test]
    fn bar_start_falls_back_to_the_creation_day() {
        let mut task = Task::new(TaskId::new(1), "bar".to_owned(), 1, ts());
        assert_eq!(task.bar_start(), ts().date_naive());

        let start = NaiveDate::from_ymd_opt(2026, 8, 10).expect("valid date");
        task.set_schedule(Some(start), None).expect("set start");
        assert_eq!(task.bar_start(), start);
    }

    #[test]
    fn invalid_apply_transition_leaves_task_unchanged() {
        let mut task = Task::new(TaskId::new(1), "x".to_owned(), 1, ts());
        let result = task.apply_transition(TaskStatus::Done, ts());
        assert!(result.is_err());
        assert_eq!(task.status(), TaskStatus::Todo);
    }
}
