//! The task-workflow service: the use cases that drive Jotphant's core loop.

use chrono::{DateTime, Utc};

use crate::app::error::Error;
use crate::domain::bank::{self, BankTransactionType};
use crate::domain::ids::TaskId;
use crate::domain::repository::{
    BankRepository, SessionRepository, TaskRepository, Transactional,
};
use crate::domain::reward;
use crate::domain::session::{PomodoroSession, SessionStatus, TimerPhase};
use crate::domain::task::{Task, TaskStatus};

/// The focus duration used until configuration arrives (M2). 25 minutes.
const DEFAULT_FOCUS_SECONDS: u32 = 25 * 60;

/// Orchestrates the task workflow over injected repository ports.
///
/// `S` is any store implementing the persistence ports; the composition root injects a
/// concrete one, while tests can inject an in-memory store or a fake.
pub struct TaskService<S> {
    store: S,
}

impl<S> TaskService<S>
where
    S: TaskRepository + SessionRepository + BankRepository + Transactional,
{
    /// Creates a service over `store`.
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Creates a new `Todo` task.
    ///
    /// # Errors
    /// Returns [`Error`] if the task cannot be persisted.
    pub fn create_task(
        &self,
        title: &str,
        estimated_pomos: u32,
        now: DateTime<Utc>,
    ) -> Result<Task, Error> {
        let task = self.store.create_task(title, estimated_pomos, now)?;
        Ok(task)
    }

    /// Moves a task into progress and starts a focus session.
    ///
    /// Enforces the single-active-task invariant: if a *different* task is already
    /// active, the call is rejected (auto-pause-on-switch arrives in M1).
    ///
    /// # Errors
    /// Returns [`Error::TaskNotFound`] if the task does not exist,
    /// [`Error::TaskAlreadyActive`] if another task is active, [`Error::Transition`] if
    /// the task cannot move to in-progress, or a storage error.
    pub fn start_task(&self, task_id: TaskId, now: DateTime<Utc>) -> Result<Task, Error> {
        if let Some(active) = self.store.find_active_task()?
            && active.id() != task_id
        {
            return Err(Error::TaskAlreadyActive);
        }

        let mut task = self.store.get_task(task_id)?.ok_or(Error::TaskNotFound)?;
        task.apply_transition(TaskStatus::InProgress, now)?;

        self.store.transaction(|| {
            self.store.update_task(&task)?;
            self.store
                .create_session(task_id, TimerPhase::Focus, DEFAULT_FOCUS_SECONDS, now)?;
            Ok(())
        })?;
        Ok(task)
    }

    /// Records the task's running focus session as completed (a pomo reached zero).
    ///
    /// # Errors
    /// Returns [`Error::NoRunningPomo`] if the task has no running focus session, or a
    /// storage error.
    pub fn complete_active_pomo(&self, task_id: TaskId, now: DateTime<Utc>) -> Result<(), Error> {
        let mut sessions = self.store.list_sessions_for_task(task_id)?;
        let session = sessions
            .iter_mut()
            .find(|session| {
                session.status() == SessionStatus::Running && session.phase() == TimerPhase::Focus
            })
            .ok_or(Error::NoRunningPomo)?;
        session.complete(now);
        self.store.update_session(session)?;
        Ok(())
    }

    /// Completes a task, banking its earned pomos.
    ///
    /// Atomically: any running session is abandoned (partial time discarded), the task is
    /// marked done, and a `TaskReward` credit for the completed focus pomos is appended.
    /// Domain validation happens before the transaction so only the writes are atomic.
    /// Returns the number of earned pomos.
    ///
    /// # Errors
    /// Returns [`Error::TaskNotFound`], [`Error::TaskNotActive`] if the task is not
    /// in-progress or paused, [`Error::Transition`], [`Error::RewardOverflow`], or a
    /// storage error.
    pub fn complete_task(&self, task_id: TaskId, now: DateTime<Utc>) -> Result<u32, Error> {
        let mut task = self.store.get_task(task_id)?.ok_or(Error::TaskNotFound)?;
        if !matches!(task.status(), TaskStatus::InProgress | TaskStatus::Paused) {
            return Err(Error::TaskNotActive);
        }

        let mut sessions = self.store.list_sessions_for_task(task_id)?;
        let earned = reward::completed_focus_pomos(&sessions);
        let amount = i32::try_from(earned).map_err(|_| Error::RewardOverflow)?;
        task.apply_transition(TaskStatus::Done, now)?;

        // Abandon any still-running session; its partial time is discarded and does not
        // count (it was never Completed, so `earned` above is unaffected).
        let abandoned: Vec<PomodoroSession> = sessions
            .iter_mut()
            .filter(|session| session.status() == SessionStatus::Running)
            .map(|session| {
                session.abandon(now);
                session.clone()
            })
            .collect();

        self.store.transaction(|| {
            for session in &abandoned {
                self.store.update_session(session)?;
            }
            self.store.update_task(&task)?;
            if amount > 0 {
                self.store.append_transaction(
                    Some(task_id),
                    amount,
                    BankTransactionType::TaskReward,
                    now,
                )?;
            }
            Ok(())
        })?;
        Ok(earned)
    }

    /// Lists all tasks.
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn list_tasks(&self) -> Result<Vec<Task>, Error> {
        let tasks = self.store.list_tasks()?;
        Ok(tasks)
    }

    /// Returns the current pomo bank balance.
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn bank_balance(&self) -> Result<i64, Error> {
        let ledger = self.store.list_transactions()?;
        Ok(bank::balance(&ledger))
    }

    /// Returns the task's running focus session, if any (used by the UI countdown).
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn active_focus_session(
        &self,
        task_id: TaskId,
    ) -> Result<Option<PomodoroSession>, Error> {
        let sessions = self.store.list_sessions_for_task(task_id)?;
        Ok(sessions.into_iter().rev().find(|session| {
            session.status() == SessionStatus::Running && session.phase() == TimerPhase::Focus
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SqliteStore;

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).expect("valid timestamp")
    }

    fn service() -> TaskService<SqliteStore> {
        TaskService::new(SqliteStore::open_in_memory().expect("in-memory store"))
    }

    #[test]
    fn create_task_starts_in_todo() {
        let service = service();
        let task = service.create_task("plan", 3, ts()).expect("create");
        assert_eq!(task.status(), TaskStatus::Todo);
        assert_eq!(task.estimated_pomos(), 3);
    }

    #[test]
    fn start_task_activates_and_opens_a_focus_session() {
        let service = service();
        let task = service.create_task("do work", 2, ts()).expect("create");
        let started = service.start_task(task.id(), ts()).expect("start");
        assert_eq!(started.status(), TaskStatus::InProgress);

        let session = service
            .active_focus_session(task.id())
            .expect("query")
            .expect("running session");
        assert_eq!(session.phase(), TimerPhase::Focus);
        assert_eq!(session.status(), SessionStatus::Running);
    }

    #[test]
    fn start_task_rejects_a_second_active_task() {
        let service = service();
        let first = service.create_task("first", 1, ts()).expect("create");
        let second = service.create_task("second", 1, ts()).expect("create");
        service.start_task(first.id(), ts()).expect("start first");

        let error = service
            .start_task(second.id(), ts())
            .expect_err("only one active task allowed");
        assert!(matches!(error, Error::TaskAlreadyActive));
    }

    #[test]
    fn start_missing_task_is_not_found() {
        let service = service();
        let error = service
            .start_task(TaskId::new(999), ts())
            .expect_err("missing task");
        assert!(matches!(error, Error::TaskNotFound));
    }

    #[test]
    fn completed_task_cannot_be_restarted() {
        let service = service();
        let task = service.create_task("one and done", 1, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service
            .complete_active_pomo(task.id(), ts())
            .expect("complete pomo");
        service.complete_task(task.id(), ts()).expect("complete");

        let error = service
            .start_task(task.id(), ts())
            .expect_err("done is terminal");
        assert!(matches!(error, Error::Transition(_)));
    }

    #[test]
    fn complete_task_banks_completed_focus_pomos() {
        let service = service();
        let task = service.create_task("ship it", 2, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service
            .complete_active_pomo(task.id(), ts())
            .expect("complete pomo");

        let earned = service.complete_task(task.id(), ts()).expect("complete task");
        assert_eq!(earned, 1);
        assert_eq!(service.bank_balance().expect("balance"), 1);

        let done = service
            .list_tasks()
            .expect("list")
            .into_iter()
            .find(|candidate| candidate.id() == task.id())
            .expect("task exists");
        assert_eq!(done.status(), TaskStatus::Done);
    }

    #[test]
    fn completing_with_a_running_pomo_earns_nothing_and_abandons_it() {
        let service = service();
        let task = service.create_task("abandon me", 1, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");

        let earned = service.complete_task(task.id(), ts()).expect("complete");
        assert_eq!(earned, 0);
        assert_eq!(service.bank_balance().expect("balance"), 0);
        assert!(
            service
                .active_focus_session(task.id())
                .expect("query")
                .is_none()
        );
    }

    #[test]
    fn cannot_complete_a_task_that_is_not_active() {
        let service = service();
        let task = service.create_task("idle", 1, ts()).expect("create");
        let error = service
            .complete_task(task.id(), ts())
            .expect_err("todo task is not active");
        assert!(matches!(error, Error::TaskNotActive));
    }
}
