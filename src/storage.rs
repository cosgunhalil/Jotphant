//! Persistence layer: SQLite adapters implementing the repository ports defined by
//! [`crate::domain::repository`].

pub mod config;
pub mod notes;
pub mod schema;
pub mod sqlite;

pub use sqlite::SqliteStore;
