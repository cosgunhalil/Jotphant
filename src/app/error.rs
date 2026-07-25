//! Application-service errors.

use crate::domain::repository::RepositoryError;
use crate::domain::task::InvalidTransition;

/// An error from an application service operation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No task exists with the given id.
    #[error("task not found")]
    TaskNotFound,
    /// No note exists with the given id.
    #[error("note not found")]
    NoteNotFound,
    /// The operation requires an active (in-progress or paused) task.
    #[error("task is not active")]
    TaskNotActive,
    /// The task has no running session to advance.
    #[error("no running session for the task")]
    NoRunningSession,
    /// The earned pomo count did not fit the ledger's amount type.
    #[error("reward amount is too large")]
    RewardOverflow,
    /// A task state transition was rejected.
    #[error(transparent)]
    Transition(#[from] InvalidTransition),
    /// The storage backend failed.
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}
