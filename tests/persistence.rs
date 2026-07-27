//! Integration tests exercising the public API against a real, file-backed database.
//!
//! These complement the in-module unit tests (which use an in-memory database) by
//! covering persistence across a store reopen and the real on-disk file path.

use chrono::{DateTime, Utc};
use jotphant::app::TaskService;
use jotphant::domain::{AppConfig, Language, PomodoroConfig, TaskStatus, ThemeChoice};
use jotphant::storage::{SqliteStore, config};
use tempfile::tempdir;

fn ts() -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000, 0).expect("valid timestamp")
}

fn open_service(db: &std::path::Path) -> TaskService<SqliteStore> {
    TaskService::new(
        SqliteStore::open(db).expect("open database"),
        AppConfig::default(),
    )
}

#[test]
fn task_lifecycle_and_reward_persist_across_reopen() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("jotphant.db");

    let task_id = {
        let service = open_service(&db);
        let task = service.create_task("persist me", 2, ts()).expect("create");
        service.start_task(task.id(), ts()).expect("start");
        service.advance_pomodoro(task.id(), ts()).expect("advance");
        let earned = service.complete_task(task.id(), ts()).expect("complete");
        assert_eq!(earned, 1);
        task.id()
    }; // store (and its connection) dropped here

    // Reopen the same file with a fresh store.
    let service = open_service(&db);
    assert_eq!(service.bank_balance().expect("balance"), 1);
    let task = service
        .list_tasks()
        .expect("list")
        .into_iter()
        .find(|task| task.id() == task_id)
        .expect("task exists");
    assert_eq!(task.status(), TaskStatus::Done);
}

#[test]
fn notes_tags_and_jots_persist_across_reopen() {
    let dir = tempdir().expect("temp dir");
    let db = dir.path().join("jotphant.db");

    let (note_id, task_id) = {
        let service = open_service(&db);
        let note = service
            .create_note("Idea", "some **body**", ts())
            .expect("create note");
        service
            .set_note_tags(note.id(), &["work".to_owned(), "urgent".to_owned()])
            .expect("tag");
        let task = service.create_task("work", 1, ts()).expect("create task");
        service
            .quick_jot(task.id(), "remember this", ts())
            .expect("jot");
        (note.id(), task.id())
    };

    let service = open_service(&db);
    assert!(
        service
            .list_notes()
            .expect("notes")
            .iter()
            .any(|note| note.id() == note_id)
    );
    assert_eq!(
        service.note_tags(note_id).expect("tags"),
        ["urgent", "work"]
    );
    let jots = service.task_notes(task_id).expect("jots");
    assert_eq!(jots.len(), 1);
    assert_eq!(jots[0].body_markdown(), "remember this");
    assert_eq!(jots[0].task_id(), Some(task_id));
}

#[test]
fn config_round_trips_on_disk() {
    let dir = tempdir().expect("temp dir");
    let path = dir.path().join("config.toml");

    // First access writes the provided first-run default.
    let defaults = config::load_or_create(&path, AppConfig::default()).expect("create");
    assert_eq!(defaults, AppConfig::default());
    assert!(path.exists());

    // A customized config survives a save/reload.
    let custom = AppConfig::new(
        PomodoroConfig::new(50 * 60, 10 * 60, 20 * 60, 3, false, true),
        8,
        ThemeChoice::Dark,
        Language::English,
    );
    config::save(&path, &custom).expect("save");
    assert_eq!(
        config::load_or_create(&path, AppConfig::default()).expect("reload"),
        custom
    );
}
