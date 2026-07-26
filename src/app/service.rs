//! The task-workflow service: the use cases that drive Jotphant's core loop.

use chrono::{DateTime, Utc};

use crate::app::error::Error;
use crate::domain::bank::{self, BankTransactionType};
use crate::domain::config::AppConfig;
use crate::domain::ids::{NoteId, TaskId};
use crate::domain::note::Note;
use crate::domain::repository::{
    BankRepository, NoteRepository, SessionRepository, TaskRepository, Transactional,
};
use crate::domain::reward;
use crate::domain::session::{PomodoroSession, SessionStatus, TimerPhase};
use crate::domain::task::{Task, TaskStatus};
use crate::domain::wikilink;

/// Derives a note title from quick-jot text: its first non-empty line (truncated), or
/// `"Quick note"` if there is none.
fn quick_jot_title(text: &str) -> String {
    let first = text.lines().next().unwrap_or("").trim();
    if first.is_empty() {
        "Quick note".to_owned()
    } else {
        first.chars().take(80).collect()
    }
}

/// Orchestrates the task workflow over injected repository ports.
///
/// `S` is any store implementing the persistence ports; the composition root injects a
/// concrete one, while tests can inject an in-memory store or a fake.
pub struct TaskService<S> {
    store: S,
    config: AppConfig,
}

impl<S> TaskService<S>
where
    S: TaskRepository + SessionRepository + BankRepository + NoteRepository + Transactional,
{
    /// Creates a service over `store` with the given application configuration.
    pub fn new(store: S, config: AppConfig) -> Self {
        Self { store, config }
    }

    /// The configured leisure minutes earned per banked pomo.
    #[must_use]
    pub fn leisure_minutes_per_pomo(&self) -> u32 {
        self.config.leisure_minutes_per_pomo()
    }

    /// The current application configuration.
    #[must_use]
    pub fn config(&self) -> AppConfig {
        self.config
    }

    /// Replaces the configuration, applying it to subsequent operations.
    pub fn set_config(&mut self, config: AppConfig) {
        self.config = config;
    }

    /// Creates a new `Todo` task.
    ///
    /// # Errors
    /// Returns [`Error`] if the task cannot be persisted.
    pub fn create_task(
        &self,
        title: &str,
        estimated_pomos: u32,
        now: DateTime<Utc>,
    ) -> Result<Task, Error> {
        let task = self.store.create_task(title, estimated_pomos, now)?;
        Ok(task)
    }

    /// Moves a task into progress and starts a focus session.
    ///
    /// Preserves the single-active-task invariant by **auto-pausing** any other active
    /// task first (abandoning its running pomo), then activating the requested one — all
    /// in one transaction.
    ///
    /// # Errors
    /// Returns [`Error::TaskNotFound`] if the task does not exist, [`Error::Transition`]
    /// if the task cannot move to in-progress, or a storage error.
    pub fn start_task(&self, task_id: TaskId, now: DateTime<Utc>) -> Result<Task, Error> {
        let mut task = self.store.get_task(task_id)?.ok_or(Error::TaskNotFound)?;
        task.apply_transition(TaskStatus::InProgress, now)?;

        // If a different task is active, pause it and abandon its running pomo.
        let paused = match self.store.find_active_task()? {
            Some(mut active) if active.id() != task_id => {
                active.apply_transition(TaskStatus::Paused, now)?;
                let abandoned = self.collect_abandoned_running(active.id(), now)?;
                Some((active, abandoned))
            }
            _ => None,
        };

        self.store.transaction(|| {
            if let Some((active, abandoned)) = &paused {
                for session in abandoned {
                    self.store.update_session(session)?;
                }
                self.store.update_task(active)?;
            }
            self.store.update_task(&task)?;
            self.store.create_session(
                task_id,
                TimerPhase::Focus,
                self.config.pomodoro().duration_seconds(TimerPhase::Focus),
                now,
            )?;
            Ok(())
        })?;
        Ok(task)
    }

    /// Advances the Pomodoro cycle: completes the task's running session and, per the
    /// config's auto-start policy, starts the next phase (focus → break → focus …).
    ///
    /// Called both when a phase's timer reaches zero and when the user skips a break.
    ///
    /// # Errors
    /// Returns [`Error::NoRunningSession`] if the task has no running session, or a
    /// storage error.
    pub fn advance_pomodoro(&self, task_id: TaskId, now: DateTime<Utc>) -> Result<(), Error> {
        let mut sessions = self.store.list_sessions_for_task(task_id)?;
        let Some(index) = sessions
            .iter()
            .position(|session| session.status() == SessionStatus::Running)
        else {
            return Err(Error::NoRunningSession);
        };

        let completed_phase = sessions[index].phase();
        sessions[index].complete(now);
        let completed_session = sessions[index].clone();

        let completed_focus = reward::completed_focus_pomos(&sessions);
        let pomodoro = self.config.pomodoro();
        let next_phase = pomodoro.next_phase(completed_phase, completed_focus);
        let auto_start = pomodoro.should_auto_start(next_phase);
        let duration = pomodoro.duration_seconds(next_phase);

        self.store.transaction(|| {
            self.store.update_session(&completed_session)?;
            if auto_start {
                self.store
                    .create_session(task_id, next_phase, duration, now)?;
            }
            Ok(())
        })?;
        Ok(())
    }

    /// Completes a task, banking its earned pomos.
    ///
    /// Atomically: any running session is abandoned (partial time discarded), the task is
    /// marked done, and a `TaskReward` credit for the completed focus pomos is appended.
    /// Domain validation happens before the transaction so only the writes are atomic.
    /// Returns the number of earned pomos.
    ///
    /// # Errors
    /// Returns [`Error::TaskNotFound`], [`Error::TaskNotActive`] if the task is not
    /// in-progress or paused, [`Error::Transition`], [`Error::RewardOverflow`], or a
    /// storage error.
    pub fn complete_task(&self, task_id: TaskId, now: DateTime<Utc>) -> Result<u32, Error> {
        let mut task = self.store.get_task(task_id)?.ok_or(Error::TaskNotFound)?;
        if !matches!(task.status(), TaskStatus::InProgress | TaskStatus::Paused) {
            return Err(Error::TaskNotActive);
        }

        let earned = reward::completed_focus_pomos(&self.store.list_sessions_for_task(task_id)?);
        let amount = i32::try_from(earned).map_err(|_| Error::RewardOverflow)?;
        task.apply_transition(TaskStatus::Done, now)?;

        // Abandon any still-running session; its partial time is discarded and does not
        // count (it was never Completed, so `earned` above is unaffected).
        let abandoned = self.collect_abandoned_running(task_id, now)?;

        self.store.transaction(|| {
            for session in &abandoned {
                self.store.update_session(session)?;
            }
            self.store.update_task(&task)?;
            if amount > 0 {
                self.store.append_transaction(
                    Some(task_id),
                    amount,
                    BankTransactionType::TaskReward,
                    now,
                )?;
            }
            Ok(())
        })?;
        Ok(earned)
    }

    /// Suspends the active task, abandoning its running pomo (partial time discarded).
    ///
    /// # Errors
    /// Returns [`Error::TaskNotFound`], [`Error::Transition`] if the task is not
    /// in-progress, or a storage error.
    pub fn pause_task(&self, task_id: TaskId, now: DateTime<Utc>) -> Result<Task, Error> {
        let mut task = self.store.get_task(task_id)?.ok_or(Error::TaskNotFound)?;
        task.apply_transition(TaskStatus::Paused, now)?;
        let abandoned = self.collect_abandoned_running(task_id, now)?;

        self.store.transaction(|| {
            for session in &abandoned {
                self.store.update_session(session)?;
            }
            self.store.update_task(&task)?;
            Ok(())
        })?;
        Ok(task)
    }

    /// Cancels a task. Its running pomo is abandoned and its unbanked pomos are
    /// discarded (no reward); completed sessions remain in history.
    ///
    /// # Errors
    /// Returns [`Error::TaskNotFound`], [`Error::Transition`] if the task is neither
    /// in-progress nor paused, or a storage error.
    pub fn cancel_task(&self, task_id: TaskId, now: DateTime<Utc>) -> Result<Task, Error> {
        let mut task = self.store.get_task(task_id)?.ok_or(Error::TaskNotFound)?;
        task.apply_transition(TaskStatus::Cancelled, now)?;
        let abandoned = self.collect_abandoned_running(task_id, now)?;

        self.store.transaction(|| {
            for session in &abandoned {
                self.store.update_session(session)?;
            }
            self.store.update_task(&task)?;
            Ok(())
        })?;
        Ok(task)
    }

    /// Abandons the task's running sessions (in memory) and returns the updated copies to
    /// persist. Their partial time is discarded and they never count as effort.
    fn collect_abandoned_running(
        &self,
        task_id: TaskId,
        now: DateTime<Utc>,
    ) -> Result<Vec<PomodoroSession>, Error> {
        let mut sessions = self.store.list_sessions_for_task(task_id)?;
        let abandoned = sessions
            .iter_mut()
            .filter(|session| session.status() == SessionStatus::Running)
            .map(|session| {
                session.abandon(now);
                session.clone()
            })
            .collect();
        Ok(abandoned)
    }

    /// Replaces a task's description, returning the updated task.
    ///
    /// # Errors
    /// Returns [`Error::TaskNotFound`] if the task does not exist, or a storage error.
    pub fn set_task_description(
        &self,
        task_id: TaskId,
        description: String,
    ) -> Result<Task, Error> {
        let mut task = self.store.get_task(task_id)?.ok_or(Error::TaskNotFound)?;
        task.set_description(description);
        self.store.update_task(&task)?;
        Ok(task)
    }

    /// The phase that is awaiting a manual start, if the active task is in progress but
    /// has no running session (because the previous phase completed with auto-start off).
    ///
    /// # Errors
    /// Returns a storage error if a query fails.
    pub fn pending_next_phase(&self, task_id: TaskId) -> Result<Option<TimerPhase>, Error> {
        let Some(task) = self.store.get_task(task_id)? else {
            return Ok(None);
        };
        if task.status() != TaskStatus::InProgress {
            return Ok(None);
        }
        let sessions = self.store.list_sessions_for_task(task_id)?;
        if sessions
            .iter()
            .any(|session| session.status() == SessionStatus::Running)
        {
            return Ok(None);
        }
        let completed_focus = reward::completed_focus_pomos(&sessions);
        let next = sessions
            .iter()
            .max_by_key(|session| session.id().value())
            .map_or(TimerPhase::Focus, |session| {
                self.config
                    .pomodoro()
                    .next_phase(session.phase(), completed_focus)
            });
        Ok(Some(next))
    }

    /// Starts the pending next phase for a task (used when auto-start is off).
    ///
    /// A no-op if nothing is pending (no active task, or a session already runs).
    ///
    /// # Errors
    /// Returns a storage error if the query or insert fails.
    pub fn start_next_phase(&self, task_id: TaskId, now: DateTime<Utc>) -> Result<(), Error> {
        let Some(next) = self.pending_next_phase(task_id)? else {
            return Ok(());
        };
        let duration = self.config.pomodoro().duration_seconds(next);
        self.store.create_session(task_id, next, duration, now)?;
        Ok(())
    }

    /// Catches up a timer that elapsed while the app was closed.
    ///
    /// If the active task has a running session whose configured duration has already
    /// passed, its phase is completed and the next one started (as if the timer had fired
    /// at the moment it expired). The next phase starts fresh from `now`, so time spent
    /// with the app closed is not replayed as extra phases. A no-op if nothing is active
    /// or the running timer has time left.
    ///
    /// # Errors
    /// Returns a storage error, or a transition error while advancing.
    pub fn reconcile_active_timer(&self, now: DateTime<Utc>) -> Result<(), Error> {
        let Some(active) = self.store.find_active_task()? else {
            return Ok(());
        };
        if let Some(session) = self.running_session(active.id())?
            && session.is_expired(now)
        {
            self.advance_pomodoro(active.id(), now)?;
        }
        Ok(())
    }

    // --- Notes ---

    /// Creates a new standalone note.
    ///
    /// # Errors
    /// Returns a storage error if the insert fails.
    pub fn create_note(
        &self,
        title: &str,
        body_markdown: &str,
        now: DateTime<Utc>,
    ) -> Result<Note, Error> {
        let note = self.store.create_note(title, body_markdown, None, now)?;
        self.update_links_for(&note)?;
        Ok(note)
    }

    /// Lists all notes (pinned first, then most recently updated).
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn list_notes(&self) -> Result<Vec<Note>, Error> {
        let notes = self.store.list_notes()?;
        Ok(notes)
    }

    /// Searches notes by title/body substring.
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn search_notes(&self, query: &str) -> Result<Vec<Note>, Error> {
        let notes = self.store.search_notes(query)?;
        Ok(notes)
    }

    /// Updates a note's title and body, bumping its updated-at time.
    ///
    /// # Errors
    /// Returns [`Error::NoteNotFound`] if the note does not exist, or a storage error.
    pub fn save_note_content(
        &self,
        id: NoteId,
        title: String,
        body_markdown: String,
        now: DateTime<Utc>,
    ) -> Result<Note, Error> {
        let mut note = self.store.get_note(id)?.ok_or(Error::NoteNotFound)?;
        note.set_title(title);
        note.set_body(body_markdown);
        note.touch(now);
        self.store.update_note(&note)?;
        self.update_links_for(&note)?;
        Ok(note)
    }

    /// Parses `[[wiki-links]]` from a note's body, resolves them to existing notes by
    /// title, and stores the resulting outgoing links.
    fn update_links_for(&self, note: &Note) -> Result<(), Error> {
        let targets = wikilink::extract_links(note.body_markdown());
        let ids = if targets.is_empty() {
            Vec::new()
        } else {
            let all = self.store.list_notes()?;
            targets
                .iter()
                .filter_map(|target| {
                    all.iter()
                        .find(|candidate| {
                            candidate.title() == target.as_str() && candidate.id() != note.id()
                        })
                        .map(Note::id)
                })
                .collect()
        };
        self.store.set_note_links(note.id(), &ids)?;
        Ok(())
    }

    /// Creates a note attached to a task (the quick-jot bridge). The first line becomes
    /// the title; the whole text is the body.
    ///
    /// # Errors
    /// Returns a storage error if the insert fails.
    pub fn quick_jot(
        &self,
        task_id: TaskId,
        text: &str,
        now: DateTime<Utc>,
    ) -> Result<Note, Error> {
        let title = quick_jot_title(text);
        let note = self.store.create_note(&title, text, Some(task_id), now)?;
        self.update_links_for(&note)?;
        Ok(note)
    }

    /// Returns the notes attached to a task (its jots), newest first.
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn task_notes(&self, task_id: TaskId) -> Result<Vec<Note>, Error> {
        let notes = self.store.list_notes_for_task(task_id)?;
        Ok(notes)
    }

    /// Returns the notes that link to the given note (its backlinks).
    ///
    /// # Errors
    /// Returns a storage error if a query fails.
    pub fn note_backlinks(&self, id: NoteId) -> Result<Vec<Note>, Error> {
        let ids = self.store.backlinks(id)?;
        let mut notes = Vec::new();
        for backlink_id in ids {
            if let Some(note) = self.store.get_note(backlink_id)? {
                notes.push(note);
            }
        }
        Ok(notes)
    }

    /// Sets a note's pinned flag.
    ///
    /// # Errors
    /// Returns [`Error::NoteNotFound`] if the note does not exist, or a storage error.
    pub fn set_note_pinned(
        &self,
        id: NoteId,
        pinned: bool,
        now: DateTime<Utc>,
    ) -> Result<(), Error> {
        let mut note = self.store.get_note(id)?.ok_or(Error::NoteNotFound)?;
        note.set_pinned(pinned);
        note.touch(now);
        self.store.update_note(&note)?;
        Ok(())
    }

    /// Sets a note's archived flag.
    ///
    /// # Errors
    /// Returns [`Error::NoteNotFound`] if the note does not exist, or a storage error.
    pub fn set_note_archived(
        &self,
        id: NoteId,
        archived: bool,
        now: DateTime<Utc>,
    ) -> Result<(), Error> {
        let mut note = self.store.get_note(id)?.ok_or(Error::NoteNotFound)?;
        note.set_archived(archived);
        note.touch(now);
        self.store.update_note(&note)?;
        Ok(())
    }

    /// Returns a note's tags.
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn note_tags(&self, id: NoteId) -> Result<Vec<String>, Error> {
        let tags = self.store.note_tags(id)?;
        Ok(tags)
    }

    /// Replaces a note's tags.
    ///
    /// # Errors
    /// Returns a storage error if the update fails.
    pub fn set_note_tags(&self, id: NoteId, tags: &[String]) -> Result<(), Error> {
        self.store.set_note_tags(id, tags)?;
        Ok(())
    }

    /// Lists all tasks.
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn list_tasks(&self) -> Result<Vec<Task>, Error> {
        let tasks = self.store.list_tasks()?;
        Ok(tasks)
    }

    /// Returns the single active (in-progress) task, if any.
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn active_task(&self) -> Result<Option<Task>, Error> {
        let task = self.store.find_active_task()?;
        Ok(task)
    }

    /// Returns the number of completed focus pomos for a task (its measured effort).
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn completed_pomos(&self, task_id: TaskId) -> Result<u32, Error> {
        let sessions = self.store.list_sessions_for_task(task_id)?;
        Ok(reward::completed_focus_pomos(&sessions))
    }

    /// Returns the current pomo bank balance.
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn bank_balance(&self) -> Result<i64, Error> {
        let ledger = self.store.list_transactions()?;
        Ok(bank::balance(&ledger))
    }

    /// Returns the task's running session of any phase, if any (used by the UI countdown).
    ///
    /// # Errors
    /// Returns a storage error if the query fails.
    pub fn running_session(&self, task_id: TaskId) -> Result<Option<PomodoroSession>, Error> {
        let sessions = self.store.list_sessions_for_task(task_id)?;
        Ok(sessions
            .into_iter()
            .rev()
            .find(|session| session.status() == SessionStatus::Running))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pomodoro::PomodoroConfig;
    use crate::storage::SqliteStore;

    fn ts() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 0).expect("valid timestamp")
    }

    fn service() -> TaskService<SqliteStore> {
        service_with(AppConfig::default())
    }

    fn service_with(config: AppConfig) -> TaskService<SqliteStore> {
        TaskService::new(
            SqliteStore::open_in_memory().expect("in-memory store"),
            config,
        )
    }

    #[test]
    fn create_task_starts_in_todo() {
        let service = service();
        let task = service.create_task("plan", 3, ts()).expect("create");
        assert_eq!(task.status(), TaskStatus::Todo);
        assert_eq!(task.estimated_pomos(), 3);
    }

    #[test]
    fn start_task_activates_and_opens_a_focus_session() {
        let service = service();
        let task = service.create_task("do work", 2, ts()).expect("create");
        let started = service.start_task(task.id(), ts()).expect("start");
        assert_eq!(started.status(), TaskStatus::InProgress);

        let session = service
            .running_session(task.id())
            .expect("query")
            .expect("running session");
        assert_eq!(session.phase(), TimerPhase::Focus);
        assert_eq!(session.status(), SessionStatus::Running);
    }

    #[test]
    fn starting_another_task_auto_pauses_the_active_one() {
        let service = service();
        let first = service.create_task("first", 1, ts()).expect("create");
        let second = service.create_task("second", 1, ts()).expect("create");
        service.start_task(first.id(), ts()).expect("start first");

        let started = service.start_task(second.id(), ts()).expect("start second");
        assert_eq!(started.status(), TaskStatus::InProgress);

        // The previously active task is now paused, and its running pomo was abandoned.
        let first_now = service
            .list_tasks()
            .expect("list")
            .into_iter()
            .find(|candidate| candidate.id() == first.id())
            .expect("first exists");
        assert_eq!(first_now.status(), TaskStatus::Paused);
        assert!(
            service
                .running_session(first.id())
                .expect("query")
                .is_none()
        );

        // Exactly one task is active, and it is the second one.
        let active = service
            .active_task()
            .expect("query")
            .expect("an active task");
        assert_eq!(active.id(), second.id());
    }

    #[test]
    fn start_missing_task_is_not_found() {
        let service = service();
        let error = service
            .start_task(TaskId::new(999), ts())
            .expect_err("missing task");
        assert!(matches!(error, Error::TaskNotFound));
    }

    #[test]
    fn completed_task_cannot_be_restarted() {
        let service = service();
        let task = service
            .create_task("one and done", 1, ts())
            .expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service
            .advance_pomodoro(task.id(), ts())
            .expect("complete pomo");
        service.complete_task(task.id(), ts()).expect("complete");

        let error = service
            .start_task(task.id(), ts())
            .expect_err("done is terminal");
        assert!(matches!(error, Error::Transition(_)));
    }

    #[test]
    fn complete_task_banks_completed_focus_pomos() {
        let service = service();
        let task = service.create_task("ship it", 2, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service
            .advance_pomodoro(task.id(), ts())
            .expect("complete pomo");

        let earned = service
            .complete_task(task.id(), ts())
            .expect("complete task");
        assert_eq!(earned, 1);
        assert_eq!(service.bank_balance().expect("balance"), 1);

        let done = service
            .list_tasks()
            .expect("list")
            .into_iter()
            .find(|candidate| candidate.id() == task.id())
            .expect("task exists");
        assert_eq!(done.status(), TaskStatus::Done);
    }

    #[test]
    fn completing_with_a_running_pomo_earns_nothing_and_abandons_it() {
        let service = service();
        let task = service.create_task("abandon me", 1, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");

        let earned = service.complete_task(task.id(), ts()).expect("complete");
        assert_eq!(earned, 0);
        assert_eq!(service.bank_balance().expect("balance"), 0);
        assert!(service.running_session(task.id()).expect("query").is_none());
    }

    #[test]
    fn pause_task_suspends_and_abandons_running_pomo() {
        let service = service();
        let task = service.create_task("pause me", 2, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");

        let paused = service.pause_task(task.id(), ts()).expect("pause");
        assert_eq!(paused.status(), TaskStatus::Paused);
        assert!(service.running_session(task.id()).expect("query").is_none());
    }

    #[test]
    fn paused_task_can_be_resumed() {
        let service = service();
        let task = service.create_task("resume me", 2, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service.pause_task(task.id(), ts()).expect("pause");

        let resumed = service.start_task(task.id(), ts()).expect("resume");
        assert_eq!(resumed.status(), TaskStatus::InProgress);
        assert!(service.running_session(task.id()).expect("query").is_some());
    }

    #[test]
    fn earned_pomos_accumulate_across_a_pause() {
        let service = service();
        let task = service
            .create_task("two sittings", 2, ts())
            .expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service
            .advance_pomodoro(task.id(), ts())
            .expect("first pomo");
        service.pause_task(task.id(), ts()).expect("pause");
        service.start_task(task.id(), ts()).expect("resume");
        service
            .advance_pomodoro(task.id(), ts())
            .expect("second pomo");

        let earned = service.complete_task(task.id(), ts()).expect("complete");
        assert_eq!(earned, 2);
        assert_eq!(service.bank_balance().expect("balance"), 2);
    }

    #[test]
    fn cannot_pause_a_todo_task() {
        let service = service();
        let task = service.create_task("still todo", 1, ts()).expect("create");
        let error = service
            .pause_task(task.id(), ts())
            .expect_err("only in-progress tasks pause");
        assert!(matches!(error, Error::Transition(_)));
    }

    #[test]
    fn cancel_task_marks_cancelled_without_reward() {
        let service = service();
        let task = service
            .create_task("abandon ship", 2, ts())
            .expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service
            .advance_pomodoro(task.id(), ts())
            .expect("one pomo done");

        let cancelled = service.cancel_task(task.id(), ts()).expect("cancel");
        assert_eq!(cancelled.status(), TaskStatus::Cancelled);
        assert_eq!(service.bank_balance().expect("balance"), 0);
    }

    #[test]
    fn cancel_from_paused_is_allowed() {
        let service = service();
        let task = service.create_task("shelve it", 2, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service.pause_task(task.id(), ts()).expect("pause");

        let cancelled = service.cancel_task(task.id(), ts()).expect("cancel");
        assert_eq!(cancelled.status(), TaskStatus::Cancelled);
    }

    #[test]
    fn cancelled_task_keeps_measured_effort_but_earns_no_reward() {
        let service = service();
        let task = service.create_task("half done", 4, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service
            .advance_pomodoro(task.id(), ts())
            .expect("one focus pomo -> break");

        service.cancel_task(task.id(), ts()).expect("cancel");

        // The completed focus pomo remains as measured effort (history)...
        assert_eq!(service.completed_pomos(task.id()).expect("effort"), 1);
        // ...but it generated no bank reward.
        assert_eq!(service.bank_balance().expect("balance"), 0);
    }

    #[test]
    fn cannot_cancel_a_todo_task() {
        let service = service();
        let task = service
            .create_task("never started", 1, ts())
            .expect("create");
        let error = service
            .cancel_task(task.id(), ts())
            .expect_err("todo cannot be cancelled");
        assert!(matches!(error, Error::Transition(_)));
    }

    #[test]
    fn notes_can_be_created_edited_and_tagged() {
        let service = service();
        let note = service
            .create_note("Idea", "first draft", ts())
            .expect("create");
        assert!(!note.pinned());

        let saved = service
            .save_note_content(
                note.id(),
                "Idea v2".to_owned(),
                "second draft".to_owned(),
                ts(),
            )
            .expect("save");
        assert_eq!(saved.title(), "Idea v2");
        assert_eq!(saved.body_markdown(), "second draft");

        service.set_note_pinned(note.id(), true, ts()).expect("pin");
        service
            .set_note_tags(note.id(), &["work".to_owned()])
            .expect("tag");
        assert_eq!(service.note_tags(note.id()).expect("tags"), ["work"]);

        let listed = service.list_notes().expect("list");
        assert_eq!(listed.len(), 1);
        assert!(listed[0].pinned());
    }

    #[test]
    fn wikilinks_in_a_note_body_create_backlinks() {
        let service = service();
        let target = service
            .create_note("Target", "", ts())
            .expect("create target");
        let source = service
            .create_note("Source", "", ts())
            .expect("create source");

        service
            .save_note_content(
                source.id(),
                "Source".to_owned(),
                "see [[Target]]".to_owned(),
                ts(),
            )
            .expect("save source");

        let backlinks = service.note_backlinks(target.id()).expect("backlinks");
        assert_eq!(backlinks.len(), 1);
        assert_eq!(backlinks[0].id(), source.id());

        // A dangling link to a non-existent title is simply ignored.
        service
            .save_note_content(
                source.id(),
                "Source".to_owned(),
                "[[Nope]]".to_owned(),
                ts(),
            )
            .expect("save with dangling link");
        assert!(
            service
                .note_backlinks(target.id())
                .expect("backlinks")
                .is_empty()
        );
    }

    #[test]
    fn quick_jot_attaches_a_note_to_the_task() {
        let service = service();
        let task = service.create_task("work", 1, ts()).expect("create");

        let note = service
            .quick_jot(task.id(), "first line\nmore detail", ts())
            .expect("jot");
        assert_eq!(note.task_id(), Some(task.id()));
        assert_eq!(note.title(), "first line");
        assert_eq!(note.body_markdown(), "first line\nmore detail");

        let listed = service.list_notes().expect("list");
        assert!(
            listed
                .iter()
                .any(|candidate| candidate.id() == note.id()
                    && candidate.task_id() == Some(task.id()))
        );
    }

    #[test]
    fn task_notes_lists_jots_newest_first() {
        let service = service();
        let task = service.create_task("work", 1, ts()).expect("task");
        service.quick_jot(task.id(), "one", ts()).expect("jot one");
        service.quick_jot(task.id(), "two", ts()).expect("jot two");

        let notes = service.task_notes(task.id()).expect("notes");
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].body_markdown(), "two");
        assert_eq!(notes[1].body_markdown(), "one");
    }

    #[test]
    fn saving_a_missing_note_is_not_found() {
        let service = service();
        let error = service
            .save_note_content(NoteId::new(123), "x".to_owned(), String::new(), ts())
            .expect_err("missing note");
        assert!(matches!(error, Error::NoteNotFound));
    }

    #[test]
    fn set_task_description_persists() {
        let service = service();
        let task = service.create_task("describe me", 1, ts()).expect("create");
        assert_eq!(task.description(), "");

        let updated = service
            .set_task_description(task.id(), "the full story".to_owned())
            .expect("set description");
        assert_eq!(updated.description(), "the full story");

        let reloaded = service
            .list_tasks()
            .expect("list")
            .into_iter()
            .find(|candidate| candidate.id() == task.id())
            .expect("exists");
        assert_eq!(reloaded.description(), "the full story");
    }

    #[test]
    fn completing_a_focus_auto_starts_a_short_break() {
        let service = service();
        let task = service.create_task("cycle", 4, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");

        service.advance_pomodoro(task.id(), ts()).expect("advance");
        let running = service
            .running_session(task.id())
            .expect("query")
            .expect("a break is running");
        assert_eq!(running.phase(), TimerPhase::ShortBreak);
        assert_eq!(running.status(), SessionStatus::Running);
    }

    #[test]
    fn completing_a_break_auto_starts_a_focus() {
        let service = service();
        let task = service.create_task("cycle", 4, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service
            .advance_pomodoro(task.id(), ts())
            .expect("focus -> break");

        service
            .advance_pomodoro(task.id(), ts())
            .expect("break -> focus");
        let running = service
            .running_session(task.id())
            .expect("query")
            .expect("a focus is running");
        assert_eq!(running.phase(), TimerPhase::Focus);
    }

    #[test]
    fn every_fourth_focus_leads_to_a_long_break() {
        let service = service(); // default long_break_after = 4
        let task = service.create_task("cycle", 8, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");

        // Complete three focus pomos, each followed by a short break.
        for _ in 0..3 {
            service
                .advance_pomodoro(task.id(), ts())
                .expect("focus -> break");
            let running = service
                .running_session(task.id())
                .expect("query")
                .expect("break");
            assert_eq!(running.phase(), TimerPhase::ShortBreak);
            service
                .advance_pomodoro(task.id(), ts())
                .expect("break -> focus");
        }

        // The fourth focus completes -> long break.
        service
            .advance_pomodoro(task.id(), ts())
            .expect("fourth focus");
        let running = service
            .running_session(task.id())
            .expect("query")
            .expect("long break");
        assert_eq!(running.phase(), TimerPhase::LongBreak);
    }

    #[test]
    fn reconcile_completes_a_timer_that_expired_while_closed() {
        let service = service(); // default focus = 25 min
        let task = service
            .create_task("left running", 4, ts())
            .expect("create");
        service.start_task(task.id(), ts()).expect("start");

        // Reopen 40 minutes later — the focus should have finished.
        let later = ts() + chrono::Duration::seconds(40 * 60);
        service
            .reconcile_active_timer(later)
            .expect("reconcile on reopen");

        let running = service
            .running_session(task.id())
            .expect("query")
            .expect("a break is running");
        assert_eq!(running.phase(), TimerPhase::ShortBreak);
        // The new phase started fresh from `later`, not back-dated.
        assert!(!running.is_expired(later));
    }

    #[test]
    fn reconcile_leaves_a_still_running_timer_alone() {
        let service = service();
        let task = service.create_task("mid focus", 4, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");

        // Reopen 5 minutes into a 25-minute focus.
        let later = ts() + chrono::Duration::seconds(5 * 60);
        service.reconcile_active_timer(later).expect("reconcile");

        let running = service
            .running_session(task.id())
            .expect("query")
            .expect("still running");
        assert_eq!(running.phase(), TimerPhase::Focus);
    }

    #[test]
    fn reconcile_is_a_noop_without_an_active_task() {
        let service = service();
        service.create_task("idle", 1, ts()).expect("create");
        service.reconcile_active_timer(ts()).expect("reconcile");
    }

    #[test]
    fn manual_start_next_phase_when_auto_start_is_off() {
        let config = AppConfig::new(
            PomodoroConfig::new(25 * 60, 5 * 60, 15 * 60, 4, false, false),
            5,
        );
        let service = service_with(config);
        let task = service.create_task("manual", 4, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");

        // Completing the focus does not auto-start a break.
        service.advance_pomodoro(task.id(), ts()).expect("advance");
        assert!(service.running_session(task.id()).expect("query").is_none());
        assert_eq!(
            service.pending_next_phase(task.id()).expect("pending"),
            Some(TimerPhase::ShortBreak)
        );

        // Starting it manually opens the break.
        service
            .start_next_phase(task.id(), ts())
            .expect("start next");
        let running = service
            .running_session(task.id())
            .expect("query")
            .expect("break started");
        assert_eq!(running.phase(), TimerPhase::ShortBreak);
    }

    #[test]
    fn set_config_updates_the_reward_rate() {
        let mut service = service();
        assert_eq!(service.leisure_minutes_per_pomo(), 5);

        service.set_config(AppConfig::new(PomodoroConfig::default(), 10));
        assert_eq!(service.leisure_minutes_per_pomo(), 10);
    }

    #[test]
    fn advancing_with_no_running_session_errors() {
        let service = service();
        let task = service.create_task("idle", 1, ts()).expect("create");
        let error = service
            .advance_pomodoro(task.id(), ts())
            .expect_err("nothing to advance");
        assert!(matches!(error, Error::NoRunningSession));
    }

    #[test]
    fn cannot_complete_a_task_that_is_not_active() {
        let service = service();
        let task = service.create_task("idle", 1, ts()).expect("create");
        let error = service
            .complete_task(task.id(), ts())
            .expect_err("todo task is not active");
        assert!(matches!(error, Error::TaskNotActive));
    }
}
