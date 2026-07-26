//! SQLite implementation of [`NoteRepository`].

use rusqlite::{Row, params};

use crate::domain::ids::{NoteId, TaskId};
use crate::domain::note::Note;
use crate::domain::repository::{NoteRepository, RepositoryError};

use super::sqlite::{SqliteStore, datetime_from_str, datetime_to_str};

const NOTE_COLUMNS: &str =
    "id, title, body_markdown, task_id, pinned, archived, created_at, updated_at";

fn row_to_note(row: &Row) -> Result<Note, RepositoryError> {
    Ok(Note::from_fields(
        NoteId::new(row.get("id")?),
        row.get("title")?,
        row.get("body_markdown")?,
        row.get::<_, Option<i64>>("task_id")?.map(TaskId::new),
        row.get("pinned")?,
        row.get("archived")?,
        datetime_from_str(&row.get::<_, String>("created_at")?)?,
        datetime_from_str(&row.get::<_, String>("updated_at")?)?,
    ))
}

impl NoteRepository for SqliteStore {
    fn create_note(
        &self,
        title: &str,
        body_markdown: &str,
        task_id: Option<TaskId>,
        created_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<Note, RepositoryError> {
        let created = datetime_to_str(created_at);
        self.connection().execute(
            "INSERT INTO notes (title, body_markdown, task_id, pinned, archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, 0, ?4, ?4)",
            params![title, body_markdown, task_id.map(TaskId::value), created],
        )?;
        let id = NoteId::new(self.connection().last_insert_rowid());
        Ok(Note::new(
            id,
            title.to_owned(),
            body_markdown.to_owned(),
            task_id,
            created_at,
        ))
    }

    fn get_note(&self, id: NoteId) -> Result<Option<Note>, RepositoryError> {
        let sql = format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?1");
        let mut stmt = self.connection().prepare(&sql)?;
        let mut rows = stmt.query(params![id.value()])?;
        match rows.next()? {
            Some(row) => Ok(Some(row_to_note(row)?)),
            None => Ok(None),
        }
    }

    fn list_notes(&self) -> Result<Vec<Note>, RepositoryError> {
        let sql = format!("SELECT {NOTE_COLUMNS} FROM notes ORDER BY pinned DESC, updated_at DESC");
        let mut stmt = self.connection().prepare(&sql)?;
        let mut rows = stmt.query([])?;
        let mut notes = Vec::new();
        while let Some(row) = rows.next()? {
            notes.push(row_to_note(row)?);
        }
        Ok(notes)
    }

    fn search_notes(&self, query: &str) -> Result<Vec<Note>, RepositoryError> {
        let sql = format!(
            "SELECT {NOTE_COLUMNS} FROM notes
             WHERE title LIKE ?1 OR body_markdown LIKE ?1
             ORDER BY pinned DESC, updated_at DESC"
        );
        let pattern = format!("%{query}%");
        let mut stmt = self.connection().prepare(&sql)?;
        let mut rows = stmt.query(params![pattern])?;
        let mut notes = Vec::new();
        while let Some(row) = rows.next()? {
            notes.push(row_to_note(row)?);
        }
        Ok(notes)
    }

    fn list_notes_for_task(&self, task_id: TaskId) -> Result<Vec<Note>, RepositoryError> {
        let sql = format!("SELECT {NOTE_COLUMNS} FROM notes WHERE task_id = ?1 ORDER BY id DESC");
        let mut stmt = self.connection().prepare(&sql)?;
        let mut rows = stmt.query(params![task_id.value()])?;
        let mut notes = Vec::new();
        while let Some(row) = rows.next()? {
            notes.push(row_to_note(row)?);
        }
        Ok(notes)
    }

    fn update_note(&self, note: &Note) -> Result<(), RepositoryError> {
        let affected = self.connection().execute(
            "UPDATE notes
             SET title = ?2, body_markdown = ?3, task_id = ?4, pinned = ?5,
                 archived = ?6, updated_at = ?7
             WHERE id = ?1",
            params![
                note.id().value(),
                note.title(),
                note.body_markdown(),
                note.task_id().map(TaskId::value),
                note.pinned(),
                note.archived(),
                datetime_to_str(note.updated_at()),
            ],
        )?;
        if affected == 0 {
            return Err(RepositoryError::NotFound);
        }
        Ok(())
    }

    fn set_note_tags(&self, note_id: NoteId, tags: &[String]) -> Result<(), RepositoryError> {
        let tx = self.connection().unchecked_transaction()?;
        tx.execute(
            "DELETE FROM note_tags WHERE note_id = ?1",
            params![note_id.value()],
        )?;
        {
            let mut stmt =
                tx.prepare("INSERT OR IGNORE INTO note_tags (note_id, tag) VALUES (?1, ?2)")?;
            for tag in tags {
                stmt.execute(params![note_id.value(), tag])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn note_tags(&self, note_id: NoteId) -> Result<Vec<String>, RepositoryError> {
        let mut stmt = self
            .connection()
            .prepare("SELECT tag FROM note_tags WHERE note_id = ?1 ORDER BY tag")?;
        let mut rows = stmt.query(params![note_id.value()])?;
        let mut tags = Vec::new();
        while let Some(row) = rows.next()? {
            tags.push(row.get::<_, String>(0)?);
        }
        Ok(tags)
    }

    fn set_note_links(
        &self,
        from_note_id: NoteId,
        to_note_ids: &[NoteId],
    ) -> Result<(), RepositoryError> {
        let tx = self.connection().unchecked_transaction()?;
        tx.execute(
            "DELETE FROM note_links WHERE from_note_id = ?1",
            params![from_note_id.value()],
        )?;
        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO note_links (from_note_id, to_note_id) VALUES (?1, ?2)",
            )?;
            for to in to_note_ids {
                stmt.execute(params![from_note_id.value(), to.value()])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn backlinks(&self, to_note_id: NoteId) -> Result<Vec<NoteId>, RepositoryError> {
        let mut stmt = self.connection().prepare(
            "SELECT from_note_id FROM note_links WHERE to_note_id = ?1 ORDER BY from_note_id",
        )?;
        let mut rows = stmt.query(params![to_note_id.value()])?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next()? {
            ids.push(NoteId::new(row.get::<_, i64>(0)?));
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::domain::repository::TaskRepository;

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).expect("valid timestamp")
    }

    fn store() -> SqliteStore {
        SqliteStore::open_in_memory().expect("in-memory store")
    }

    #[test]
    fn create_then_get_round_trips_a_note() {
        let store = store();
        let created = store
            .create_note("Title", "Some **markdown**", None, ts())
            .expect("create");
        let fetched = store.get_note(created.id()).expect("get").expect("exists");
        assert_eq!(fetched, created);
        assert_eq!(fetched.body_markdown(), "Some **markdown**");
        assert!(!fetched.pinned());
    }

    #[test]
    fn list_orders_pinned_first() {
        let store = store();
        store.create_note("a", "", None, ts()).expect("create a");
        let mut b = store.create_note("b", "", None, ts()).expect("create b");
        b.set_pinned(true);
        store.update_note(&b).expect("pin b");

        let titles: Vec<String> = store
            .list_notes()
            .expect("list")
            .iter()
            .map(|note| note.title().to_owned())
            .collect();
        assert_eq!(titles.first().map(String::as_str), Some("b"));
    }

    #[test]
    fn update_persists_body_and_flags() {
        let store = store();
        let mut note = store.create_note("t", "old", None, ts()).expect("create");
        note.set_body("new body".to_owned());
        note.set_archived(true);
        note.touch(ts());
        store.update_note(&note).expect("update");

        let fetched = store.get_note(note.id()).expect("get").expect("exists");
        assert_eq!(fetched.body_markdown(), "new body");
        assert!(fetched.archived());
    }

    #[test]
    fn update_unknown_note_is_not_found() {
        let store = store();
        let ghost = Note::new(NoteId::new(999), "x".to_owned(), String::new(), None, ts());
        assert!(matches!(
            store.update_note(&ghost),
            Err(RepositoryError::NotFound)
        ));
    }

    #[test]
    fn search_matches_title_and_body() {
        let store = store();
        store
            .create_note("Shopping", "buy milk", None, ts())
            .expect("create");
        store
            .create_note("Ideas", "a milkshake stand", None, ts())
            .expect("create");
        store
            .create_note("Unrelated", "nothing here", None, ts())
            .expect("create");

        let hits = store.search_notes("milk").expect("search");
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn tags_are_replaced_wholesale() {
        let store = store();
        let note = store.create_note("t", "", None, ts()).expect("create");
        store
            .set_note_tags(note.id(), &["work".to_owned(), "urgent".to_owned()])
            .expect("set tags");
        assert_eq!(
            store.note_tags(note.id()).expect("tags"),
            ["urgent", "work"]
        );

        store
            .set_note_tags(note.id(), &["home".to_owned()])
            .expect("replace tags");
        assert_eq!(store.note_tags(note.id()).expect("tags"), ["home"]);
    }

    #[test]
    fn lists_notes_for_a_task_newest_first() {
        let store = store();
        let task = store.create_task("t", 1, ts()).expect("task");
        store
            .create_note("first", "1", Some(task.id()), ts())
            .expect("n1");
        store
            .create_note("second", "2", Some(task.id()), ts())
            .expect("n2");
        store
            .create_note("unrelated", "x", None, ts())
            .expect("n3");

        let notes = store.list_notes_for_task(task.id()).expect("list");
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].title(), "second");
        assert_eq!(notes[1].title(), "first");
    }

    #[test]
    fn links_produce_backlinks() {
        let store = store();
        let a = store.create_note("a", "", None, ts()).expect("create a");
        let b = store.create_note("b", "", None, ts()).expect("create b");
        let target = store
            .create_note("target", "", None, ts())
            .expect("create target");

        store
            .set_note_links(a.id(), &[target.id()])
            .expect("link a");
        store
            .set_note_links(b.id(), &[target.id()])
            .expect("link b");

        let mut backlinks = store.backlinks(target.id()).expect("backlinks");
        backlinks.sort_by_key(|id| id.value());
        assert_eq!(backlinks, vec![a.id(), b.id()]);
        assert!(store.backlinks(a.id()).expect("none").is_empty());
    }
}
