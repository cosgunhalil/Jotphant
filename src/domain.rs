//! Pure domain layer: entities, enums, the task state machine, and reward math.
//!
//! This module stays free of I/O, GUI, and database concerns (see
//! `CODING_STANDARDS.md` §2). Time is never read from the wall clock here; callers pass
//! timestamps in.

pub mod bank;
pub mod ids;
pub mod repository;
pub mod reward;
pub mod session;
pub mod task;

pub use bank::{BankTransaction, BankTransactionType};
pub use ids::{BankTransactionId, PomodoroSessionId, TaskId};
pub use repository::{BankRepository, RepositoryError, SessionRepository, TaskRepository};
pub use session::{PomodoroSession, SessionStatus, TimerPhase};
pub use task::{InvalidTransition, Task, TaskStatus};
