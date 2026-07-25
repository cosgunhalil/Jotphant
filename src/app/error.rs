//! Application-service errors.

use crate::domain::repository::RepositoryError;
use crate::domain::task::InvalidTransition;

/// An error from an application service operation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No task exists with the given id.
    #[error("task not found")]
    TaskNotFound,
    /// The operation requires an active (in-progress or paused) task.
    #[error("task is not active")]
    TaskNotActive,
    /// The task has no running focus session to complete.
    #[error("no running focus session for the task")]
    NoRunningPomo,
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
