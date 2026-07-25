//! Notes: the independent Markdown notebook.
//!
//! Notes stand alone; the only tie to a task is an optional `task_id`, set solely by the
//! quick-jot flow (see `SCOPE.md`).

use chrono::{DateTime, Utc};

use crate::domain::ids::{NoteId, TaskId};

/// A Markdown note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    id: NoteId,
    title: String,
    body_markdown: String,
    task_id: Option<TaskId>,
    pinned: bool,
    archived: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl Note {
    /// Creates a new note (not pinned, not archived); `updated_at` starts at `created_at`.
    #[must_use]
    pub fn new(
        id: NoteId,
        title: String,
        body_markdown: String,
        task_id: Option<TaskId>,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            title,
            body_markdown,
            task_id,
            pinned: false,
            archived: false,
            created_at,
            updated_at: created_at,
        }
    }

    /// Reconstructs a note from persisted fields.
    #[expect(clippy::too_many_arguments, reason = "hydrates a full persisted row")]
    #[must_use]
    pub fn from_fields(
        id: NoteId,
        title: String,
        body_markdown: String,
        task_id: Option<TaskId>,
        pinned: bool,
        archived: bool,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            title,
            body_markdown,
            task_id,
            pinned,
            archived,
            created_at,
            updated_at,
        }
    }

    /// The note's identifier.
    #[must_use]
    pub fn id(&self) -> NoteId {
        self.id
    }

    /// The note's title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The note's Markdown body.
    #[must_use]
    pub fn body_markdown(&self) -> &str {
        &self.body_markdown
    }

    /// The task this note was quick-jotted onto, if any.
    #[must_use]
    pub fn task_id(&self) -> Option<TaskId> {
        self.task_id
    }

    /// Whether the note is pinned.
    #[must_use]
    pub fn pinned(&self) -> bool {
        self.pinned
    }

    /// Whether the note is archived.
    #[must_use]
    pub fn archived(&self) -> bool {
        self.archived
    }

    /// When the note was created.
    #[must_use]
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    /// When the note was last updated.
    #[must_use]
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    /// Replaces the title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Replaces the Markdown body.
    pub fn set_body(&mut self, body_markdown: String) {
        self.body_markdown = body_markdown;
    }

    /// Sets the pinned flag.
    pub fn set_pinned(&mut self, pinned: bool) {
        self.pinned = pinned;
    }

    /// Sets the archived flag.
    pub fn set_archived(&mut self, archived: bool) {
        self.archived = archived;
    }

    /// Records `now` as the last-updated time.
    pub fn touch(&mut self, now: DateTime<Utc>) {
        self.updated_at = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(1000, 0).expect("valid timestamp")
    }

    #[test]
    fn new_note_is_unpinned_unarchived_and_untouched() {
        let note = Note::new(
            NoteId::new(1),
            "t".to_owned(),
            "body".to_owned(),
            None,
            ts(),
        );
        assert!(!note.pinned());
        assert!(!note.archived());
        assert_eq!(note.created_at(), ts());
        assert_eq!(note.updated_at(), ts());
        assert_eq!(note.task_id(), None);
    }

    #[test]
    fn setters_and_touch_update_fields() {
        let mut note = Note::new(NoteId::new(1), "t".to_owned(), "b".to_owned(), None, ts());
        note.set_title("new".to_owned());
        note.set_body("changed".to_owned());
        note.set_pinned(true);
        note.set_archived(true);
        let later = DateTime::from_timestamp(2000, 0).expect("valid timestamp");
        note.touch(later);

        assert_eq!(note.title(), "new");
        assert_eq!(note.body_markdown(), "changed");
        assert!(note.pinned());
        assert!(note.archived());
        assert_eq!(note.updated_at(), later);
        assert_eq!(note.created_at(), ts()); // unchanged
    }
}
