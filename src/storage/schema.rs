//! Database schema and migrations.
//!
//! Migrations are tracked with SQLite's `user_version` pragma and applied forward only.

use rusqlite::Connection;

use crate::domain::repository::RepositoryError;

/// The schema version this build expects.
const SCHEMA_VERSION: i64 = 4;

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

/// Version 4: optional effort/effect ratings on tasks (the value matrix).
const ALTER_V4: &str = "
ALTER TABLE tasks ADD COLUMN effort TEXT;
ALTER TABLE tasks ADD COLUMN effect TEXT;
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
    if version < 4 {
        conn.execute_batch(ALTER_V4)?;
    }
    if version < SCHEMA_VERSION {
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(conn: &Connection) -> i64 {
        conn.pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("read user_version")
    }

    #[test]
    fn migrates_a_fresh_database_to_current() {
        let conn = Connection::open_in_memory().expect("open");
        migrate(&conn).expect("migrate");
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        // All tables usable.
        assert!(conn.prepare("SELECT description FROM tasks").is_ok());
        assert!(conn.prepare("SELECT effort, effect FROM tasks").is_ok());
        assert!(conn.prepare("SELECT id FROM notes").is_ok());
        assert!(conn.prepare("SELECT tag FROM note_tags").is_ok());
    }

    #[test]
    fn upgrades_a_v1_database_to_current() {
        let conn = Connection::open_in_memory().expect("open");
        // Simulate a database created by an older (v1) build.
        conn.execute_batch(CREATE_V1).expect("create v1");
        conn.pragma_update(None, "user_version", 1_i64)
            .expect("set version");
        // A v1 database has no description column and no notes table.
        assert!(conn.prepare("SELECT description FROM tasks").is_err());
        assert!(conn.prepare("SELECT id FROM notes").is_err());

        migrate(&conn).expect("upgrade");

        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        assert!(conn.prepare("SELECT description FROM tasks").is_ok());
        assert!(conn.prepare("SELECT effort, effect FROM tasks").is_ok());
        assert!(conn.prepare("SELECT id FROM notes").is_ok());
    }

    #[test]
    fn migrate_is_idempotent() {
        let conn = Connection::open_in_memory().expect("open");
        migrate(&conn).expect("first");
        migrate(&conn).expect("second");
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
    }
}
