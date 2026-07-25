//! Database schema and migrations.
//!
//! Migrations are tracked with SQLite's `user_version` pragma and applied forward only.

use rusqlite::Connection;

use crate::domain::repository::RepositoryError;

/// The schema version this build expects.
const SCHEMA_VERSION: i64 = 2;

/// Version 1: the core tasks / sessions / bank tables.
const CREATE_V1: &str = r"
CREATE TABLE tasks (
    id                   INTEGER PRIMARY KEY,
    title                TEXT    NOT NULL,
    status               TEXT    NOT NULL,
    estimated_pomos      INTEGER NOT NULL,
    linked_from_task_id  INTEGER REFERENCES tasks(id),
    created_at           TEXT    NOT NULL,
    completed_at         TEXT
);

CREATE TABLE pomodoro_sessions (
    id                          INTEGER PRIMARY KEY,
    task_id                     INTEGER NOT NULL REFERENCES tasks(id),
    phase                       TEXT    NOT NULL,
    status                      TEXT    NOT NULL,
    configured_duration_seconds INTEGER NOT NULL,
    started_at                  TEXT    NOT NULL,
    finished_at                 TEXT
);

CREATE TABLE bank_transactions (
    id                INTEGER PRIMARY KEY,
    task_id           INTEGER REFERENCES tasks(id),
    amount_pomos      INTEGER NOT NULL,
    transaction_type  TEXT    NOT NULL,
    created_at        TEXT    NOT NULL
);
";

/// Version 2: add a free-form task description.
const ALTER_V2: &str = "ALTER TABLE tasks ADD COLUMN description TEXT NOT NULL DEFAULT '';";

/// Applies any pending migrations to `conn`, forward only.
///
/// # Errors
/// Returns [`RepositoryError`] if reading the version or applying a migration fails.
pub fn migrate(conn: &Connection) -> Result<(), RepositoryError> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version < 1 {
        conn.execute_batch(CREATE_V1)?;
    }
    if version < 2 {
        conn.execute_batch(ALTER_V2)?;
    }
    if version < SCHEMA_VERSION {
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    Ok(())
}
