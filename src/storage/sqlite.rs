//! The SQLite-backed store and its repository implementations.

use std::path::Path;

use chrono::{DateTime, Utc};
use rusqlite::{Connection, Row, params};

use crate::domain::bank::{BankTransaction, BankTransactionType};
use crate::domain::ids::{BankTransactionId, PomodoroSessionId, TaskId};
use crate::domain::repository::{
    BankRepository, RepositoryError, SessionRepository, TaskRepository,
};
use crate::domain::session::{PomodoroSession, SessionStatus, TimerPhase};
use crate::domain::task::{Task, TaskStatus};

use super::schema;

impl From<rusqlite::Error> for RepositoryError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Backend {
            message: error.to_string(),
        }
    }
}

/// A SQLite-backed store owning a single connection.
///
/// All repository methods take `&self`; SQLite serializes access, and this suits the
/// single-user desktop app. The connection is opened with foreign keys enabled and the
/// schema migrated up to date.
pub struct SqliteStore {
    conn: Connection,
}

impl SqliteStore {
    /// Opens (or creates) a database at `path`, enabling foreign keys and migrating.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the database cannot be opened or migrated.
    pub fn open(path: &Path) -> Result<Self, RepositoryError> {
        Self::init(Connection::open(path)?)
    }

    /// Opens a fresh in-memory database (used by tests), migrated and ready.
    ///
    /// # Errors
    /// Returns [`RepositoryError`] if the database cannot be opened or migrated.
    pub fn open_in_memory() -> Result<Self, RepositoryError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, RepositoryError> {
        conn.pragma_update(None, "foreign_keys", true)?;
        schema::migrate(&conn)?;
        Ok(Self { conn })
    }
}

// --- Column lists (kept in one place so SELECTs and row mappers stay in sync) ---

const TASK_COLUMNS: &str =
    "id, title, status, estimated_pomos, linked_from_task_id, created_at, completed_at";
const SESSION_COLUMNS: &str =
    "id, task_id, phase, status, configured_duration_seconds, started_at, finished_at";
const BANK_COLUMNS: &str = "id, task_id, amount_pomos, transaction_type, created_at";

// --- Value conversions at the storage boundary ---

fn task_status_to_str(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => "todo",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Paused => "paused",
        TaskStatus::Done => "done",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn task_status_from_str(value: &str) -> Result<TaskStatus, RepositoryError> {
    match value {
        "todo" => Ok(TaskStatus::Todo),
        "in_progress" => Ok(TaskStatus::InProgress),
        "paused" => Ok(TaskStatus::Paused),
        "done" => Ok(TaskStatus::Done),
        "cancelled" => Ok(TaskStatus::Cancelled),
        other => Err(unknown("task status", other)),
    }
}

fn phase_to_str(phase: TimerPhase) -> &'static str {
    match phase {
        TimerPhase::Focus => "focus",
        TimerPhase::ShortBreak => "short_break",
        TimerPhase::LongBreak => "long_break",
    }
}

fn phase_from_str(value: &str) -> Result<TimerPhase, RepositoryError> {
    match value {
        "focus" => Ok(TimerPhase::Focus),
        "short_break" => Ok(TimerPhase::ShortBreak),
        "long_break" => Ok(TimerPhase::LongBreak),
        other => Err(unknown("timer phase", other)),
    }
}

fn session_status_to_str(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Running => "running",
        SessionStatus::Completed => "completed",
        SessionStatus::Abandoned => "abandoned",
    }
}

fn session_status_from_str(value: &str) -> Result<SessionStatus, RepositoryError> {
    match value {
        "running" => Ok(SessionStatus::Running),
        "completed" => Ok(SessionStatus::Completed),
        "abandoned" => Ok(SessionStatus::Abandoned),
        other => Err(unknown("session status", other)),
    }
}

fn bank_type_to_str(transaction_type: BankTransactionType) -> &'static str {
    match transaction_type {
        BankTransactionType::TaskReward => "task_reward",
    }
}

fn bank_type_from_str(value: &str) -> Result<BankTransactionType, RepositoryError> {
    match value {
        "task_reward" => Ok(BankTransactionType::TaskReward),
        other => Err(unknown("bank transaction type", other)),
    }
}

fn datetime_to_str(value: DateTime<Utc>) -> String {
    value.to_rfc3339()
}

fn datetime_from_str(value: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|error| RepositoryError::Backend {
            message: format!("invalid timestamp {value:?}: {error}"),
        })
}

fn unknown(kind: &str, value: &str) -> RepositoryError {
    RepositoryError::Backend {
        message: format!("unknown {kind} value {value:?}"),
    }
}

fn to_u32(value: i64) -> Result<u32, RepositoryError> {
    u32::try_from(value).map_err(|error| RepositoryError::Backend {
        message: error.to_string(),
    })
}

fn to_i32(value: i64) -> Result<i32, RepositoryError> {
    i32::try_from(value).map_err(|error| RepositoryError::Backend {
        message: error.to_string(),
    })
}

// --- Row mappers ---

fn row_to_task(row: &Row) -> Result<Task, RepositoryError> {
    let completed_at = row
        .get::<_, Option<String>>("completed_at")?
        .map(|value| datetime_from_str(&value))
        .transpose()?;
    Ok(Task::from_fields(
        TaskId::new(row.get("id")?),
        row.get("title")?,
        task_status_from_str(&row.get::<_, String>("status")?)?,
        to_u32(row.get("estimated_pomos")?)?,
        row.get::<_, Option<i64>>("linked_from_task_id")?
            .map(TaskId::new),
        datetime_from_str(&row.get::<_, String>("created_at")?)?,
        completed_at,
    ))
}

fn row_to_session(row: &Row) -> Result<PomodoroSession, RepositoryError> {
    let finished_at = row
        .get::<_, Option<String>>("finished_at")?
        .map(|value| datetime_from_str(&value))
        .transpose()?;
    Ok(PomodoroSession::from_fields(
        PomodoroSessionId::new(row.get("id")?),
        TaskId::new(row.get("task_id")?),
        phase_from_str(&row.get::<_, String>("phase")?)?,
        session_status_from_str(&row.get::<_, String>("status")?)?,
        to_u32(row.get("configured_duration_seconds")?)?,
        datetime_from_str(&row.get::<_, String>("started_at")?)?,
        finished_at,
    ))
}

fn row_to_bank_transaction(row: &Row) -> Result<BankTransaction, RepositoryError> {
    Ok(BankTransaction::new(
        BankTransactionId::new(row.get("id")?),
        row.get::<_, Option<i64>>("task_id")?.map(TaskId::new),
        to_i32(row.get("amount_pomos")?)?,
        bank_type_from_str(&row.get::<_, String>("transaction_type")?)?,
        datetime_from_str(&row.get::<_, String>("created_at")?)?,
    ))
}

impl TaskRepository for SqliteStore {
    fn create_task(
        &self,
        title: &str,
        estimated_pomos: u32,
        created_at: DateTime<Utc>,
    ) -> Result<Task, RepositoryError> {
        self.conn.execute(
            "INSERT INTO tasks (title, status, estimated_pomos, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                title,
                task_status_to_str(TaskStatus::Todo),
                i64::from(estimated_pomos),
                datetime_to_str(created_at),
            ],
        )?;
        let id = TaskId::new(self.conn.last_insert_rowid());
        Ok(Task::new(id, title.to_owned(), estimated_pomos, created_at))
    }

    fn get_task(&self, id: TaskId) -> Result<Option<Task>, RepositoryError> {
        let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![id.value()])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_task(row)?)),
            None => Ok(None),
        }
    }

    fn list_tasks(&self) -> Result<Vec<Task>, RepositoryError> {
        let sql = format!("SELECT {TASK_COLUMNS} FROM tasks ORDER BY id");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        let mut tasks = Vec::new();
        while let Some(row) = rows.next()? {
            tasks.push(row_to_task(row)?);
        }
        Ok(tasks)
    }

    fn find_active_task(&self) -> Result<Option<Task>, RepositoryError> {
        let sql = format!("SELECT {TASK_COLUMNS} FROM tasks WHERE status = ?1 LIMIT 1");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![task_status_to_str(TaskStatus::InProgress)])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_task(row)?)),
            None => Ok(None),
        }
    }

    fn update_task(&self, task: &Task) -> Result<(), RepositoryError> {
        let affected = self.conn.execute(
            "UPDATE tasks
             SET title = ?2, status = ?3, estimated_pomos = ?4,
                 linked_from_task_id = ?5, created_at = ?6, completed_at = ?7
             WHERE id = ?1",
            params![
                task.id().value(),
                task.title(),
                task_status_to_str(task.status()),
                i64::from(task.estimated_pomos()),
                task.linked_from().map(TaskId::value),
                datetime_to_str(task.created_at()),
                task.completed_at().map(datetime_to_str),
            ],
        )?;
        if affected == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }
}

impl SessionRepository for SqliteStore {
    fn create_session(
        &self,
        task_id: TaskId,
        phase: TimerPhase,
        configured_duration_seconds: u32,
        started_at: DateTime<Utc>,
    ) -> Result<PomodoroSession, RepositoryError> {
        self.conn.execute(
            "INSERT INTO pomodoro_sessions
                (task_id, phase, status, configured_duration_seconds, started_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                task_id.value(),
                phase_to_str(phase),
                session_status_to_str(SessionStatus::Running),
                i64::from(configured_duration_seconds),
                datetime_to_str(started_at),
            ],
        )?;
        let id = PomodoroSessionId::new(self.conn.last_insert_rowid());
        Ok(PomodoroSession::new(
            id,
            task_id,
            phase,
            configured_duration_seconds,
            started_at,
        ))
    }

    fn update_session(&self, session: &PomodoroSession) -> Result<(), RepositoryError> {
        let affected = self.conn.execute(
            "UPDATE pomodoro_sessions
             SET task_id = ?2, phase = ?3, status = ?4,
                 configured_duration_seconds = ?5, started_at = ?6, finished_at = ?7
             WHERE id = ?1",
            params![
                session.id().value(),
                session.task_id().value(),
                phase_to_str(session.phase()),
                session_status_to_str(session.status()),
                i64::from(session.configured_duration_seconds()),
                datetime_to_str(session.started_at()),
                session.finished_at().map(datetime_to_str),
            ],
        )?;
        if affected == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    fn list_sessions_for_task(
        &self,
        task_id: TaskId,
    ) -> Result<Vec<PomodoroSession>, RepositoryError> {
        let sql =
            format!("SELECT {SESSION_COLUMNS} FROM pomodoro_sessions WHERE task_id = ?1 ORDER BY id");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query(params![task_id.value()])?;
        let mut sessions = Vec::new();
        while let Some(row) = rows.next()? {
            sessions.push(row_to_session(row)?);
        }
        Ok(sessions)
    }
}

impl BankRepository for SqliteStore {
    fn append_transaction(
        &self,
        task_id: Option<TaskId>,
        amount_pomos: i32,
        transaction_type: BankTransactionType,
        created_at: DateTime<Utc>,
    ) -> Result<BankTransaction, RepositoryError> {
        self.conn.execute(
            "INSERT INTO bank_transactions (task_id, amount_pomos, transaction_type, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                task_id.map(TaskId::value),
                i64::from(amount_pomos),
                bank_type_to_str(transaction_type),
                datetime_to_str(created_at),
            ],
        )?;
        let id = BankTransactionId::new(self.conn.last_insert_rowid());
        Ok(BankTransaction::new(
            id,
            task_id,
            amount_pomos,
            transaction_type,
            created_at,
        ))
    }

    fn list_transactions(&self) -> Result<Vec<BankTransaction>, RepositoryError> {
        let sql = format!("SELECT {BANK_COLUMNS} FROM bank_transactions ORDER BY id");
        let mut stmt = self.conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        let mut transactions = Vec::new();
        while let Some(row) = rows.next()? {
            transactions.push(row_to_bank_transaction(row)?);
        }
        Ok(transactions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::bank;

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).expect("valid timestamp")
    }

    fn store() -> SqliteStore {
        SqliteStore::open_in_memory().expect("in-memory store opens")
    }

    #[test]
    fn create_then_get_round_trips_a_task() {
        let store = store();
        let created = store
            .create_task("write storage", 4, ts())
            .expect("create task");
        assert_eq!(created.status(), TaskStatus::Todo);

        let fetched = store
            .get_task(created.id())
            .expect("get task")
            .expect("task exists");
        assert_eq!(fetched, created);
        assert_eq!(fetched.title(), "write storage");
        assert_eq!(fetched.estimated_pomos(), 4);
        assert_eq!(fetched.created_at(), ts());
    }

    #[test]
    fn get_missing_task_returns_none() {
        let store = store();
        assert!(store.get_task(TaskId::new(999)).expect("get").is_none());
    }

    #[test]
    fn list_returns_all_tasks_in_order() {
        let store = store();
        store.create_task("a", 1, ts()).expect("create a");
        store.create_task("b", 2, ts()).expect("create b");
        let titles: Vec<String> = store
            .list_tasks()
            .expect("list")
            .iter()
            .map(|task| task.title().to_owned())
            .collect();
        assert_eq!(titles, vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn update_persists_status_and_completed_at() {
        let store = store();
        let mut task = store.create_task("finish me", 2, ts()).expect("create");
        task.apply_transition(TaskStatus::InProgress, ts())
            .expect("start");
        task.apply_transition(TaskStatus::Done, ts()).expect("done");
        store.update_task(&task).expect("update");

        let fetched = store.get_task(task.id()).expect("get").expect("exists");
        assert_eq!(fetched.status(), TaskStatus::Done);
        assert_eq!(fetched.completed_at(), Some(ts()));
    }

    #[test]
    fn update_unknown_task_is_not_found() {
        let store = store();
        let ghost = Task::new(TaskId::new(4242), "ghost".to_owned(), 1, ts());
        assert!(matches!(
            store.update_task(&ghost),
            Err(RepositoryError::NotFound)
        ));
    }

    #[test]
    fn find_active_returns_only_the_in_progress_task() {
        let store = store();
        store.create_task("idle", 1, ts()).expect("create idle");
        let mut active = store.create_task("busy", 1, ts()).expect("create busy");
        assert!(store.find_active_task().expect("find").is_none());

        active
            .apply_transition(TaskStatus::InProgress, ts())
            .expect("start");
        store.update_task(&active).expect("update");

        let found = store
            .find_active_task()
            .expect("find")
            .expect("active exists");
        assert_eq!(found.id(), active.id());
    }

    #[test]
    fn sessions_round_trip_and_list_by_task() {
        let store = store();
        let task = store
            .create_task("focus work", 2, ts())
            .expect("create task");
        let mut session = store
            .create_session(task.id(), TimerPhase::Focus, 1500, ts())
            .expect("create session");
        session.complete(ts());
        store.update_session(&session).expect("update session");

        let sessions = store
            .list_sessions_for_task(task.id())
            .expect("list sessions");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].status(), SessionStatus::Completed);
        assert_eq!(sessions[0].phase(), TimerPhase::Focus);
        assert_eq!(sessions[0].finished_at(), Some(ts()));
    }

    #[test]
    fn bank_appends_and_balances() {
        let store = store();
        let task = store.create_task("earn", 3, ts()).expect("create task");
        store
            .append_transaction(Some(task.id()), 3, BankTransactionType::TaskReward, ts())
            .expect("append reward");
        store
            .append_transaction(None, 5, BankTransactionType::TaskReward, ts())
            .expect("append reward");

        let ledger = store.list_transactions().expect("list ledger");
        assert_eq!(ledger.len(), 2);
        assert_eq!(bank::balance(&ledger), 8);
    }
}
