//! Repository ports: the persistence abstractions the app depends on and the storage
//! layer implements.
//!
//! Defining the ports here keeps the dependency direction inward (`storage → domain`)
//! and lets the app be tested against fake implementations. The error is deliberately
//! abstract so no backend detail (SQLite, etc.) leaks into the domain.

use chrono::{DateTime, Utc};

use crate::domain::bank::{BankTransaction, BankTransactionType};
use crate::domain::ids::TaskId;
use crate::domain::session::{PomodoroSession, TimerPhase};
use crate::domain::task::Task;

/// Error returned by repository ports.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// The requested entity does not exist.
    #[error("entity not found")]
    NotFound,
    /// The storage backend failed. The message carries backend detail for diagnostics.
    #[error("storage backend error: {message}")]
    Backend {
        /// Human-readable description of the backend failure.
        message: String,
    },
}

/// Persistence operations for [`Task`]s.
///
/// Methods are entity-specific (`*_task`) so a single store can implement several
/// repository ports without method-call ambiguity.
pub trait TaskRepository {
    /// Inserts a new `Todo` task and returns it with its assigned id.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the insert fails.
    fn create_task(
        &self,
        title: &str,
        estimated_pomos: u32,
        created_at: DateTime<Utc>,
    ) -> Result<Task, RepositoryError>;

    /// Fetches a task by id, or `None` if it does not exist.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the query fails.
    fn get_task(&self, id: TaskId) -> Result<Option<Task>, RepositoryError>;

    /// Lists all tasks, ordered by id.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the query fails.
    fn list_tasks(&self) -> Result<Vec<Task>, RepositoryError>;

    /// Returns the single active (`InProgress`) task, if any.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the query fails.
    fn find_active_task(&self) -> Result<Option<Task>, RepositoryError>;

    /// Persists changes to an existing task.
    ///
    /// # Errors
    /// Returns [`RepositoryError::NotFound`] if no task with the given id exists, or a
    /// backend error otherwise.
    fn update_task(&self, task: &Task) -> Result<(), RepositoryError>;
}

/// Persistence operations for [`PomodoroSession`]s.
pub trait SessionRepository {
    /// Inserts a new `Running` session and returns it with its assigned id.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the insert fails.
    fn create_session(
        &self,
        task_id: TaskId,
        phase: TimerPhase,
        configured_duration_seconds: u32,
        started_at: DateTime<Utc>,
    ) -> Result<PomodoroSession, RepositoryError>;

    /// Persists changes to an existing session.
    ///
    /// # Errors
    /// Returns [`RepositoryError::NotFound`] if no session with the given id exists, or a
    /// backend error otherwise.
    fn update_session(&self, session: &PomodoroSession) -> Result<(), RepositoryError>;

    /// Lists all sessions for a task, ordered by id.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the query fails.
    fn list_sessions_for_task(
        &self,
        task_id: TaskId,
    ) -> Result<Vec<PomodoroSession>, RepositoryError>;
}

/// Persistence operations for the bank ledger.
pub trait BankRepository {
    /// Appends a ledger entry and returns it with its assigned id.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the insert fails.
    fn append_transaction(
        &self,
        task_id: Option<TaskId>,
        amount_pomos: i32,
        transaction_type: BankTransactionType,
        created_at: DateTime<Utc>,
    ) -> Result<BankTransaction, RepositoryError>;

    /// Lists all ledger entries, ordered by id.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the query fails.
    fn list_transactions(&self) -> Result<Vec<BankTransaction>, RepositoryError>;
}
