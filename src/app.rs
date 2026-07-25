//! Application services that orchestrate [`crate::domain`] and [`crate::storage`] and
//! own the atomic task operations (start / complete task, quick-jot).

pub mod error;
pub mod service;

pub use error::Error;
pub use service::TaskService;
