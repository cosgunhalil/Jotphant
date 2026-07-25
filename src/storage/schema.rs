//! Database schema and migrations.
//!
//! Migrations are tracked with SQLite's `user_version` pragma and applied forward only.

use rusqlite::Connection;

use crate::domain::repository::RepositoryError;

/// The schema version this build expects.
const SCHEMA_VERSION: i64 = 3;

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

/// Version 3: the notes notebook (notes, their tags, and their links).
const CREATE_V3: &str = r"
CREATE TABLE notes (
    id             INTEGER PRIMARY KEY,
    title          TEXT    NOT NULL,
    body_markdown  TEXT    NOT NULL,
    task_id        INTEGER REFERENCES tasks(id),
    pinned         INTEGER NOT NULL DEFAULT 0,
    archived       INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT    NOT NULL,
    updated_at     TEXT    NOT NULL
);

CREATE TABLE note_tags (
    note_id  INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag      TEXT    NOT NULL,
    PRIMARY KEY (note_id, tag)
);

CREATE TABLE note_links (
    from_note_id  INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    to_note_id    INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    PRIMARY KEY (from_note_id, to_note_id)
);
";

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
    if version < 3 {
        conn.execute_batch(CREATE_V3)?;
    }
    if version < SCHEMA_VERSION {
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    Ok(())
}
