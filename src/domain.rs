//! Pure domain layer: entities, enums, the task state machine, and reward math.
//!
//! This module stays free of I/O, GUI, and database concerns (see
//! `CODING_STANDARDS.md` §2). Time is never read from the wall clock here; callers pass
//! timestamps in.

pub mod bank;
pub mod config;
pub mod ids;
pub mod note;
pub mod pomodoro;
pub mod repository;
pub mod reward;
pub mod session;
pub mod task;
pub mod wikilink;

pub use bank::{BankTransaction, BankTransactionType};
pub use config::{AppConfig, Language, ThemeChoice};
pub use ids::{BankTransactionId, NoteId, PomodoroSessionId, TaskId};
pub use note::Note;
pub use pomodoro::PomodoroConfig;
pub use repository::{
    BankRepository, NoteRepository, RepositoryError, SessionRepository, TaskRepository,
    Transactional,
};
pub use session::{PomodoroSession, SessionStatus, TimerPhase};
pub use task::{InvalidTransition, Task, TaskStatus};
