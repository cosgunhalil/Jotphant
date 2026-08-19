//! egui presentation layer — a Trello-style board.
//!
//! Talks only to [`crate::app::TaskService`]. Cards are grouped into status columns
//! (`Todo · In Progress · Paused · Done`; `Cancelled` is hidden). The view caches what it
//! displays and refreshes after each action rather than querying storage every frame.

pub mod theme;

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Local, NaiveDate, Utc};
use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use egui_extras::DatePickerButton;

use crate::app::{TaskEffort, TaskService};
use crate::domain::config::{AppConfig, Language, ThemeChoice};
use crate::domain::ids::{NoteId, TaskId};
use crate::domain::note::Note;
use crate::domain::pomodoro::PomodoroConfig;
use crate::domain::repository::{
    BankRepository, NoteRepository, SessionRepository, TaskRepository, Transactional,
};
use crate::domain::session::{PomodoroSession, TimerPhase};
use crate::domain::task::{Rating, Task, TaskStatus};
use crate::localization::Localizer;

/// Which top-level screen is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Board,
    Notes,
    History,
    Timeline,
}

/// Editable form state for the settings screen (durations in minutes).
struct SettingsDraft {
    focus_minutes: u32,
    short_break_minutes: u32,
    long_break_minutes: u32,
    long_break_after: u32,
    auto_start_break: bool,
    auto_start_focus: bool,
    leisure_minutes_per_pomo: u32,
    theme: ThemeChoice,
    language: Language,
}

impl SettingsDraft {
    fn from_config(config: &AppConfig) -> Self {
        let pomodoro = config.pomodoro();
        Self {
            focus_minutes: pomodoro.duration_seconds(TimerPhase::Focus) / 60,
            short_break_minutes: pomodoro.duration_seconds(TimerPhase::ShortBreak) / 60,
            long_break_minutes: pomodoro.duration_seconds(TimerPhase::LongBreak) / 60,
            long_break_after: pomodoro.long_break_after(),
            auto_start_break: pomodoro.should_auto_start(TimerPhase::ShortBreak),
            auto_start_focus: pomodoro.should_auto_start(TimerPhase::Focus),
            leisure_minutes_per_pomo: config.leisure_minutes_per_pomo(),
            theme: config.theme(),
            language: config.language(),
        }
    }

    fn to_config(&self) -> AppConfig {
        AppConfig::new(
            PomodoroConfig::new(
                self.focus_minutes.saturating_mul(60),
                self.short_break_minutes.saturating_mul(60),
                self.long_break_minutes.saturating_mul(60),
                self.long_break_after,
                self.auto_start_break,
                self.auto_start_focus,
            ),
            self.leisure_minutes_per_pomo,
            self.theme,
            self.language,
        )
    }
}

/// The columns shown on the board, in order. `Cancelled` is intentionally omitted.
const COLUMNS: [TaskStatus; 4] = [
    TaskStatus::Todo,
    TaskStatus::InProgress,
    TaskStatus::Paused,
    TaskStatus::Done,
];

/// The translation key for a task status name.
fn status_key(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => "status.todo",
        TaskStatus::InProgress => "status.in_progress",
        TaskStatus::Paused => "status.paused",
        TaskStatus::Done => "status.done",
        TaskStatus::Cancelled => "status.cancelled",
    }
}

/// A user action captured during rendering and applied afterwards, so rendering never
/// borrows `self` while a mutating service call runs.
enum Action {
    Create,
    Start(TaskId),
    Pause(TaskId),
    Cancel(TaskId),
    Advance(TaskId),
    CompleteTask(TaskId),
    Open(TaskId),
    SaveDescription(TaskId),
    CloseDetail,
    StartNext(TaskId),
    SetEstimate(TaskId),
    SetRatings(TaskId, Option<Rating>, Option<Rating>),
    SetSchedule(TaskId),
    BeginTitleEdit,
    SaveTitle(TaskId),
    CancelTitleEdit,
    CreateFollowUp(TaskId),
    OpenSettings,
    SaveSettings,
    CloseSettings,
    SwitchView(View),
    NewNote,
    SelectNote(NoteId),
    SaveNote(NoteId),
    PinNote(NoteId, bool),
    ArchiveNote(NoteId, bool),
    SearchNotes,
    QuickJot(TaskId),
    EditJot(NoteId),
    SaveJotEdit(NoteId),
    CancelJotEdit,
    DeleteJot(NoteId),
    BeginDrag(TaskId),
    DropOn(TaskStatus),
    CancelDrag,
}

/// Persists the configuration; injected by the composition root so the UI need not know
/// about storage.
type SaveConfig = Box<dyn Fn(&AppConfig) -> Result<(), String>>;

/// The root eframe application.
pub struct JotphantApp<S> {
    service: TaskService<S>,
    save_config: SaveConfig,
    new_title: String,
    new_estimate: u32,
    tasks: Vec<Task>,
    progress: HashMap<TaskId, u32>,
    balance: i64,
    leisure_per_pomo: u32,
    active_task: Option<Task>,
    active_session: Option<PomodoroSession>,
    pending_phase: Option<TimerPhase>,
    status: Option<String>,
    selected: Option<TaskId>,
    detail_description: String,
    detail_estimate: u32,
    detail_start: Option<NaiveDate>,
    detail_due: Option<NaiveDate>,
    editing_title: bool,
    detail_title: String,
    settings: Option<SettingsDraft>,
    view: View,
    notes: Vec<Note>,
    note_search: String,
    selected_note: Option<NoteId>,
    note_title: String,
    note_body: String,
    note_tags_input: String,
    note_preview: bool,
    note_backlinks: Vec<Note>,
    md_cache: CommonMarkCache,
    quick_jot_text: String,
    task_notes: Vec<Note>,
    editing_jot: Option<NoteId>,
    editing_jot_text: String,
    report: Vec<TaskEffort>,
    applied_theme: Option<ThemeChoice>,
    localizer: Localizer,
    applied_language: Language,
    dragging: Option<TaskId>,
    /// A brief celebratory highlight on a card whose pomo just completed:
    /// the task and the ui-time the flash started.
    flash: Option<(TaskId, f64)>,
}

/// How long the pomo-complete flash lasts, in seconds.
const FLASH_SECONDS: f64 = 1.2;

impl<S> JotphantApp<S>
where
    S: TaskRepository + SessionRepository + BankRepository + NoteRepository + Transactional,
{
    /// Builds the app over an injected service and config-save function, loading the
    /// initial state.
    pub fn new(service: TaskService<S>, save_config: SaveConfig) -> Self {
        let language = service.config().language();
        let mut app = Self {
            service,
            save_config,
            localizer: Localizer::new(language),
            applied_language: language,
            new_title: String::new(),
            new_estimate: 1,
            tasks: Vec::new(),
            progress: HashMap::new(),
            balance: 0,
            leisure_per_pomo: 0,
            active_task: None,
            active_session: None,
            pending_phase: None,
            status: None,
            selected: None,
            detail_description: String::new(),
            detail_estimate: 0,
            detail_start: None,
            detail_due: None,
            editing_title: false,
            detail_title: String::new(),
            settings: None,
            view: View::Board,
            notes: Vec::new(),
            note_search: String::new(),
            selected_note: None,
            note_title: String::new(),
            note_body: String::new(),
            note_tags_input: String::new(),
            note_preview: false,
            note_backlinks: Vec::new(),
            md_cache: CommonMarkCache::default(),
            quick_jot_text: String::new(),
            task_notes: Vec::new(),
            editing_jot: None,
            editing_jot_text: String::new(),
            report: Vec::new(),
            applied_theme: None,
            dragging: None,
            flash: None,
        };
        // Catch up any timer that elapsed while the app was closed, then load state.
        if let Err(error) = app.service.reconcile_active_timer(Utc::now()) {
            app.status = Some(error_message(&app.localizer, &error));
        }
        app.refresh();
        app
    }

    /// Reloads cached tasks, per-task progress, balance, and the active task/session.
    fn refresh(&mut self) {
        match self.service.list_tasks() {
            Ok(tasks) => self.tasks = tasks,
            Err(error) => self.status = Some(error_message(&self.localizer, &error)),
        }
        let mut progress = HashMap::new();
        for task in &self.tasks {
            match self.service.completed_pomos(task.id()) {
                Ok(count) => {
                    progress.insert(task.id(), count);
                }
                Err(error) => self.status = Some(error_message(&self.localizer, &error)),
            }
        }
        self.progress = progress;
        match self.service.bank_balance() {
            Ok(balance) => self.balance = balance,
            Err(error) => self.status = Some(error_message(&self.localizer, &error)),
        }
        self.leisure_per_pomo = self.service.leisure_minutes_per_pomo();
        self.active_task = match self.service.active_task() {
            Ok(task) => task,
            Err(error) => {
                self.status = Some(error_message(&self.localizer, &error));
                None
            }
        };
        let active_id = self.active_task.as_ref().map(Task::id);
        self.active_session = match active_id {
            Some(id) => match self.service.running_session(id) {
                Ok(session) => session,
                Err(error) => {
                    self.status = Some(error_message(&self.localizer, &error));
                    None
                }
            },
            None => None,
        };
        // A phase awaits a manual start only when the active task has no running session.
        self.pending_phase = match active_id {
            Some(id) if self.active_session.is_none() => {
                self.service.pending_next_phase(id).ok().flatten()
            }
            _ => None,
        };
    }

    /// Reloads the per-task effort report.
    fn refresh_report(&mut self) {
        match self.service.effort_by_task() {
            Ok(report) => self.report = report,
            Err(error) => self.status = Some(error_message(&self.localizer, &error)),
        }
    }

    /// Reloads the notes list (search results if a query is present, else all).
    fn refresh_notes(&mut self) {
        let query = self.note_search.trim();
        let result = if query.is_empty() {
            self.service.list_notes()
        } else {
            self.service.search_notes(query)
        };
        match result {
            Ok(notes) => self.notes = notes,
            Err(error) => self.status = Some(error_message(&self.localizer, &error)),
        }
    }

    /// Opens a task's detail: loads its description, estimate, schedule, and jots.
    fn open_task(&mut self, id: TaskId) {
        let task = self.tasks.iter().find(|task| task.id() == id);
        self.detail_description = task
            .map(|task| task.description().to_owned())
            .unwrap_or_default();
        self.detail_estimate = task.map_or(0, Task::estimated_pomos);
        self.detail_start = task.and_then(Task::start_date);
        self.detail_due = task.and_then(Task::due_date);
        self.detail_title = task.map(|task| task.title().to_owned()).unwrap_or_default();
        self.editing_title = false;
        self.task_notes = self.service.task_notes(id).unwrap_or_default();
        self.quick_jot_text.clear();
        self.selected = Some(id);
    }

    /// Loads a note's content and tags into the editor buffers.
    fn select_note(&mut self, id: NoteId) {
        self.selected_note = Some(id);
        if let Some(note) = self.notes.iter().find(|note| note.id() == id) {
            self.note_title = note.title().to_owned();
            self.note_body = note.body_markdown().to_owned();
        }
        self.note_tags_input = match self.service.note_tags(id) {
            Ok(tags) => tags.join(", "),
            Err(error) => {
                self.status = Some(error_message(&self.localizer, &error));
                String::new()
            }
        };
        self.note_backlinks = self.service.note_backlinks(id).unwrap_or_default();
    }

    /// Auto-advances the Pomodoro cycle once a phase's countdown reaches zero, asking
    /// the OS to flash the taskbar for attention. `ui_time` is egui's clock, used to
    /// start the completion flash.
    fn tick(&mut self, ctx: &egui::Context, now: DateTime<Utc>, ui_time: f64) {
        let expired = match (&self.active_task, &self.active_session) {
            (Some(task), Some(session)) if session.is_expired(now) => {
                Some((task.id(), session.phase()))
            }
            _ => None,
        };
        if let Some((task_id, ended_phase)) = expired {
            if let Err(error) = self.service.advance_pomodoro(task_id, now) {
                self.status = Some(error_message(&self.localizer, &error));
            }
            self.refresh();
            // Flash the taskbar so a backgrounded window still signals the transition.
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
            // Celebrate a completed focus pomo with a brief flash on its card.
            if ended_phase == TimerPhase::Focus {
                self.flash = Some((task_id, ui_time));
            }
        }
    }

    /// Applies a captured action and refreshes.
    fn handle(&mut self, action: Action, now: DateTime<Utc>) {
        let result = match action {
            Action::Open(id) => {
                self.open_task(id);
                return;
            }
            Action::BeginDrag(id) => {
                self.dragging = Some(id);
                return;
            }
            Action::CancelDrag => {
                self.dragging = None;
                return;
            }
            Action::DropOn(status) => {
                if let Some(drag_id) = self.dragging.take()
                    && let Some(drop_action) = resolve_drop(status, drag_id, &self.tasks)
                {
                    self.handle(drop_action, now);
                }
                return;
            }
            Action::SetEstimate(id) => {
                if let Err(error) = self.service.set_task_estimate(id, self.detail_estimate) {
                    self.status = Some(error_message(&self.localizer, &error));
                }
                self.refresh();
                return;
            }
            Action::SetRatings(id, effort, effect) => {
                if let Err(error) = self.service.set_task_ratings(id, effort, effect) {
                    self.status = Some(error_message(&self.localizer, &error));
                }
                self.refresh();
                return;
            }
            Action::BeginTitleEdit => {
                self.detail_title = self
                    .selected
                    .and_then(|id| self.tasks.iter().find(|task| task.id() == id))
                    .map(|task| task.title().to_owned())
                    .unwrap_or_default();
                self.editing_title = true;
                return;
            }
            Action::CancelTitleEdit => {
                self.editing_title = false;
                return;
            }
            Action::SaveTitle(id) => {
                let title = self.detail_title.trim().to_owned();
                if title.is_empty() {
                    self.status = Some(self.localizer.t("error.title_required").to_owned());
                } else if let Err(error) = self.service.set_task_title(id, title) {
                    self.status = Some(error_message(&self.localizer, &error));
                }
                self.editing_title = false;
                self.refresh();
                return;
            }
            Action::SetSchedule(id) => {
                if let Err(error) =
                    self.service
                        .set_task_schedule(id, self.detail_start, self.detail_due)
                {
                    self.status = Some(error_message(&self.localizer, &error));
                }
                self.refresh();
                // Re-sync the drafts with what was actually persisted (a rejected
                // window must not linger in the pickers).
                if let Some(task) = self.tasks.iter().find(|task| task.id() == id) {
                    self.detail_start = task.start_date();
                    self.detail_due = task.due_date();
                }
                return;
            }
            Action::CreateFollowUp(id) => {
                if let Some(source) = self.tasks.iter().find(|task| task.id() == id) {
                    let title = source.title().to_owned();
                    let estimate = source.estimated_pomos();
                    match self.service.create_follow_up(id, &title, estimate, now) {
                        Ok(new_task) => {
                            self.refresh();
                            self.open_task(new_task.id());
                        }
                        Err(error) => self.status = Some(error_message(&self.localizer, &error)),
                    }
                }
                return;
            }
            Action::CloseDetail => {
                self.selected = None;
                return;
            }
            Action::OpenSettings => {
                self.settings = Some(SettingsDraft::from_config(&self.service.config()));
                return;
            }
            Action::CloseSettings => {
                self.settings = None;
                return;
            }
            Action::SwitchView(view) => {
                self.view = view;
                match view {
                    View::Notes => self.refresh_notes(),
                    View::History => self.refresh_report(),
                    View::Board | View::Timeline => self.refresh(),
                }
                return;
            }
            Action::SearchNotes => {
                self.refresh_notes();
                return;
            }
            Action::QuickJot(id) => {
                let text = self.quick_jot_text.trim().to_owned();
                if !text.is_empty() {
                    match self.service.quick_jot(id, &text, now) {
                        Ok(_) => {
                            self.quick_jot_text.clear();
                            self.task_notes = self.service.task_notes(id).unwrap_or_default();
                        }
                        Err(error) => self.status = Some(error_message(&self.localizer, &error)),
                    }
                }
                return;
            }
            Action::EditJot(id) => {
                self.editing_jot_text = self
                    .task_notes
                    .iter()
                    .find(|note| note.id() == id)
                    .map(|note| note.body_markdown().to_owned())
                    .unwrap_or_default();
                self.editing_jot = Some(id);
                return;
            }
            Action::CancelJotEdit => {
                self.editing_jot = None;
                return;
            }
            Action::SaveJotEdit(id) => {
                let text = self.editing_jot_text.trim().to_owned();
                if !text.is_empty()
                    && let Err(error) = self.service.edit_jot(id, &text, now)
                {
                    self.status = Some(error_message(&self.localizer, &error));
                }
                self.editing_jot = None;
                if let Some(task_id) = self.selected {
                    self.task_notes = self.service.task_notes(task_id).unwrap_or_default();
                }
                return;
            }
            Action::DeleteJot(id) => {
                // "Delete" archives the jot: it leaves the comment list but stays
                // recoverable in storage.
                if let Err(error) = self.service.set_note_archived(id, true, now) {
                    self.status = Some(error_message(&self.localizer, &error));
                }
                if let Some(task_id) = self.selected {
                    self.task_notes = self.service.task_notes(task_id).unwrap_or_default();
                }
                return;
            }
            Action::NewNote => {
                let untitled = self.localizer.t("notes.untitled").to_owned();
                match self.service.create_note(&untitled, "", now) {
                    Ok(note) => {
                        self.refresh_notes();
                        self.select_note(note.id());
                    }
                    Err(error) => self.status = Some(error_message(&self.localizer, &error)),
                }
                return;
            }
            Action::SelectNote(id) => {
                self.select_note(id);
                return;
            }
            Action::SaveNote(id) => {
                let tags = parse_tags(&self.note_tags_input);
                let saved = self
                    .service
                    .save_note_content(id, self.note_title.clone(), self.note_body.clone(), now)
                    .and_then(|_| self.service.set_note_tags(id, &tags));
                if let Err(error) = saved {
                    self.status = Some(error_message(&self.localizer, &error));
                }
                self.refresh_notes();
                self.note_backlinks = self.service.note_backlinks(id).unwrap_or_default();
                return;
            }
            Action::PinNote(id, pinned) => {
                if let Err(error) = self.service.set_note_pinned(id, pinned, now) {
                    self.status = Some(error_message(&self.localizer, &error));
                }
                self.refresh_notes();
                return;
            }
            Action::ArchiveNote(id, archived) => {
                if let Err(error) = self.service.set_note_archived(id, archived, now) {
                    self.status = Some(error_message(&self.localizer, &error));
                }
                self.refresh_notes();
                return;
            }
            Action::SaveSettings => {
                if let Some(config) = self.settings.as_ref().map(SettingsDraft::to_config) {
                    self.service.set_config(config);
                    if let Err(error) = (self.save_config)(&config) {
                        self.status = Some(error);
                    }
                }
                self.settings = None;
                self.refresh();
                return;
            }
            Action::Create => {
                let title = self.new_title.trim().to_owned();
                if title.is_empty() {
                    self.status = Some(self.localizer.t("error.title_required").to_owned());
                    return;
                }
                let created = self
                    .service
                    .create_task(&title, self.new_estimate, now)
                    .map(|_| ());
                if created.is_ok() {
                    self.new_title.clear();
                }
                created
            }
            Action::Start(id) => self.service.start_task(id, now).map(|_| ()),
            Action::Pause(id) => self.service.pause_task(id, now).map(|_| ()),
            Action::Cancel(id) => self.service.cancel_task(id, now).map(|_| ()),
            Action::Advance(id) => self.service.advance_pomodoro(id, now),
            Action::StartNext(id) => self.service.start_next_phase(id, now).map(|_| ()),
            Action::CompleteTask(id) => self.service.complete_task(id, now).map(|_| ()),
            Action::SaveDescription(id) => self
                .service
                .set_task_description(id, self.detail_description.clone())
                .map(|_| ()),
        };
        self.status = result
            .err()
            .map(|error| error_message(&self.localizer, &error));
        self.refresh();
    }
}

impl<S> eframe::App for JotphantApp<S>
where
    S: TaskRepository + SessionRepository + BankRepository + NoteRepository + Transactional,
{
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Apply the configured theme on the first frame and whenever it changes.
        let theme_choice = self.service.config().theme();
        if self.applied_theme != Some(theme_choice) {
            ui.ctx().set_visuals(theme::visuals(theme_choice));
            self.applied_theme = Some(theme_choice);
        }
        // Rebuild the localizer when the configured language changes.
        let language = self.service.config().language();
        if self.applied_language != language {
            self.localizer = Localizer::new(language);
            self.applied_language = language;
        }

        let now = Utc::now();
        let today = Local::now().date_naive();
        let ui_time = ui.ctx().input(|input| input.time);
        let ctx = ui.ctx().clone();
        self.tick(&ctx, now, ui_time);

        // Expire the pomo-complete flash, repainting while it plays.
        if let Some((_, started)) = self.flash {
            if ui_time - started >= FLASH_SECONDS {
                self.flash = None;
            } else {
                ui.ctx().request_repaint();
            }
        }

        let mut action: Option<Action> = None;
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Jotphant");
                ui.separator();
                let minutes = self.balance.max(0) * i64::from(self.leisure_per_pomo);
                ui.label(self.localizer.t_args(
                    "app.bank_balance",
                    &[
                        ("pomos", self.balance.to_string()),
                        ("minutes", minutes.to_string()),
                    ],
                ));
                if ui.button(self.localizer.t("app.settings")).clicked() {
                    action = Some(Action::OpenSettings);
                }
                ui.separator();
                if ui
                    .selectable_label(self.view == View::Board, self.localizer.t("app.view_board"))
                    .clicked()
                {
                    action = Some(Action::SwitchView(View::Board));
                }
                if ui
                    .selectable_label(self.view == View::Notes, self.localizer.t("app.view_notes"))
                    .clicked()
                {
                    action = Some(Action::SwitchView(View::Notes));
                }
                if ui
                    .selectable_label(
                        self.view == View::History,
                        self.localizer.t("app.view_history"),
                    )
                    .clicked()
                {
                    action = Some(Action::SwitchView(View::History));
                }
                if ui
                    .selectable_label(
                        self.view == View::Timeline,
                        self.localizer.t("app.view_timeline"),
                    )
                    .clicked()
                {
                    action = Some(Action::SwitchView(View::Timeline));
                }
            });
            if let Some(message) = &self.status {
                ui.colored_label(egui::Color32::RED, message);
            }
            ui.separator();

            match self.view {
                View::Board => {
                    ui.columns(COLUMNS.len(), |columns| {
                        for (index, status) in COLUMNS.iter().enumerate() {
                            let ui = &mut columns[index];
                            let count = self.tasks.iter().filter(|t| t.status() == *status).count();
                            let title = self.localizer.t(status_key(*status));
                            ui.strong(format!("{title}  ({count})"));
                            ui.separator();

                            egui::ScrollArea::vertical()
                                .id_salt(status_key(*status))
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    let zone = egui::Frame::default().show(ui, |ui| {
                                        ui.set_min_height(60.0);
                                        ui.set_width(ui.available_width());
                                        let mut any = false;
                                        for task in
                                            self.tasks.iter().filter(|t| t.status() == *status)
                                        {
                                            any = true;
                                            let completed =
                                                self.progress.get(&task.id()).copied().unwrap_or(0);
                                            let is_active = Some(task.id())
                                                == self.active_task.as_ref().map(Task::id);
                                            let session = if is_active {
                                                self.active_session.as_ref()
                                            } else {
                                                None
                                            };
                                            let pending = if is_active && session.is_none() {
                                                self.pending_phase
                                            } else {
                                                None
                                            };
                                            #[expect(
                                                clippy::cast_possible_truncation,
                                                reason = "flash progress is in [0, 1]"
                                            )]
                                            let flash = self.flash.and_then(|(id, started)| {
                                                (id == task.id()).then(|| {
                                                    ((ui_time - started) / FLASH_SECONDS) as f32
                                                })
                                            });
                                            // The dragged card stays in place, dimmed;
                                            // its ghost follows the cursor.
                                            let card_action = if self.dragging == Some(task.id()) {
                                                ui.scope(|ui| {
                                                    ui.multiply_opacity(0.35);
                                                    card_ui(
                                                        ui,
                                                        &self.localizer,
                                                        task,
                                                        completed,
                                                        session,
                                                        pending,
                                                        flash,
                                                        today,
                                                        now,
                                                    )
                                                })
                                                .inner
                                            } else {
                                                card_ui(
                                                    ui,
                                                    &self.localizer,
                                                    task,
                                                    completed,
                                                    session,
                                                    pending,
                                                    flash,
                                                    today,
                                                    now,
                                                )
                                            };
                                            if let Some(card_action) = card_action {
                                                action = Some(card_action);
                                            }
                                        }
                                        if !any {
                                            ui.weak(self.localizer.t("board.drop_here"));
                                        }
                                    });

                                    // Drop-target feedback while a card is being dragged.
                                    if let Some(drag_id) = self.dragging {
                                        let rect = zone.response.rect;
                                        let valid =
                                            resolve_drop(*status, drag_id, &self.tasks).is_some();
                                        let hovered = ui
                                            .ctx()
                                            .pointer_hover_pos()
                                            .is_some_and(|pos| rect.contains(pos));
                                        let corner = egui::CornerRadius::same(8);
                                        let accent = ui.visuals().selection.stroke.color;
                                        let painter = ui.painter();
                                        if !valid {
                                            // Gray out columns that cannot accept the card.
                                            painter.rect_filled(
                                                rect,
                                                corner,
                                                ui.visuals().panel_fill.gamma_multiply(0.55),
                                            );
                                        } else if hovered {
                                            painter.rect_filled(
                                                rect,
                                                corner,
                                                accent.gamma_multiply(0.08),
                                            );
                                            painter.rect_stroke(
                                                rect,
                                                corner,
                                                egui::Stroke::new(2.0, accent),
                                                egui::StrokeKind::Inside,
                                            );
                                        } else {
                                            painter.rect_stroke(
                                                rect,
                                                corner,
                                                egui::Stroke::new(1.0, accent.gamma_multiply(0.4)),
                                                egui::StrokeKind::Inside,
                                            );
                                        }
                                        if valid
                                            && hovered
                                            && ui.ctx().input(|i| i.pointer.any_released())
                                        {
                                            action = Some(Action::DropOn(*status));
                                        }
                                    }

                                    if *status == TaskStatus::Todo {
                                        ui.separator();
                                        let new_task = ui.add(
                                            egui::TextEdit::singleline(&mut self.new_title)
                                                .hint_text(self.localizer.t("board.new_task_hint")),
                                        );
                                        // Enter submits and keeps focus for rapid entry.
                                        if new_task.lost_focus()
                                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                            && !self.new_title.trim().is_empty()
                                        {
                                            action = Some(Action::Create);
                                            new_task.request_focus();
                                        }
                                        ui.horizontal(|ui| {
                                            ui.label(self.localizer.t("board.estimate_short"));
                                            ui.add(
                                                egui::DragValue::new(&mut self.new_estimate)
                                                    .range(0..=999),
                                            );
                                            if ui.button(self.localizer.t("board.add")).clicked() {
                                                action = Some(Action::Create);
                                            }
                                        });
                                    }
                                });
                        }
                    });
                }
                View::Notes => {
                    notes_view(
                        ui,
                        &self.localizer,
                        &self.notes,
                        self.selected_note,
                        &self.note_backlinks,
                        &mut self.note_search,
                        &mut self.note_title,
                        &mut self.note_tags_input,
                        &mut self.note_body,
                        &mut self.note_preview,
                        &mut self.md_cache,
                        &mut action,
                    );
                }
                View::History => {
                    let total_effort: u32 =
                        self.report.iter().map(TaskEffort::completed_pomos).sum();
                    let minutes = self.balance.max(0) * i64::from(self.leisure_per_pomo);
                    ui.label(self.localizer.t_args(
                        "history.total_effort",
                        &[("pomos", total_effort.to_string())],
                    ));
                    ui.label(self.localizer.t_args(
                        "history.banked",
                        &[
                            ("pomos", self.balance.to_string()),
                            ("minutes", minutes.to_string()),
                        ],
                    ));
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .id_salt("history")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Grid::new("history_grid")
                                .num_columns(3)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.strong(self.localizer.t("history.column_task"));
                                    ui.strong(self.localizer.t("history.column_status"));
                                    ui.strong(self.localizer.t("history.column_pomos"));
                                    ui.end_row();
                                    for row in self.report.iter().filter(|row| {
                                        row.completed_pomos() > 0
                                            || row.task().status().is_terminal()
                                    }) {
                                        ui.label(row.task().title());
                                        ui.label(self.localizer.t(status_key(row.task().status())));
                                        ui.label(row.completed_pomos().to_string());
                                        ui.end_row();
                                    }
                                });
                        });
                }
                View::Timeline => {
                    timeline_view(ui, &self.localizer, &self.tasks, today, &mut action);
                }
            }
        });

        // Trello-style ghost: a floating copy of the dragged card follows the cursor.
        if let Some(drag_id) = self.dragging
            && let Some(task) = self.tasks.iter().find(|task| task.id() == drag_id)
            && let Some(pos) = ui.ctx().pointer_hover_pos()
        {
            let completed = self.progress.get(&drag_id).copied().unwrap_or(0);
            egui::Area::new(egui::Id::new("drag_ghost"))
                .order(egui::Order::Tooltip)
                .fixed_pos(pos + egui::vec2(14.0, 10.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(180.0);
                        ui.strong(task.title());
                        ui.weak(self.localizer.t_args(
                            "card.pomo_progress",
                            &[
                                ("completed", completed.to_string()),
                                ("estimated", task.estimated_pomos().to_string()),
                            ],
                        ));
                    });
                });
            ui.ctx()
                .output_mut(|output| output.cursor_icon = egui::CursorIcon::Grabbing);
        }
        // Releasing anywhere that is not a valid drop target cancels the drag.
        if self.dragging.is_some()
            && ui.ctx().input(|i| i.pointer.any_released())
            && !matches!(action, Some(Action::DropOn(_)))
        {
            action = Some(Action::CancelDrag);
        }

        if let Some(selected_id) = self.selected {
            let ctx = ui.ctx().clone();
            let response = egui::Modal::new(egui::Id::new("task_detail")).show(&ctx, |ui| {
                ui.set_width(700.0);
                let Some(task) = self.tasks.iter().find(|task| task.id() == selected_id) else {
                    action = Some(Action::CloseDetail);
                    return;
                };

                // Header: the title; click it to rename (Enter saves, focus loss cancels).
                if self.editing_title {
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.detail_title)
                            .desired_width(f32::INFINITY)
                            .font(egui::TextStyle::Heading),
                    );
                    if edit.lost_focus() {
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            action = Some(Action::SaveTitle(selected_id));
                        } else {
                            action = Some(Action::CancelTitleEdit);
                        }
                    }
                } else {
                    let title = egui::Label::new(egui::RichText::new(task.title()).heading())
                        .sense(egui::Sense::click());
                    if ui
                        .add(title)
                        .on_hover_cursor(egui::CursorIcon::Text)
                        .on_hover_text(self.localizer.t("detail.rename_hint"))
                        .clicked()
                    {
                        action = Some(Action::BeginTitleEdit);
                    }
                }
                ui.separator();

                ui.horizontal_top(|ui| {
                    // Left column: the content you read and write.
                    ui.vertical(|ui| {
                        ui.set_width(430.0);
                        ui.strong(self.localizer.t("detail.description"));
                        ui.add(
                            egui::TextEdit::multiline(&mut self.detail_description)
                                .desired_rows(5)
                                .desired_width(f32::INFINITY),
                        );
                        if ui
                            .button(self.localizer.t("detail.save_description"))
                            .clicked()
                        {
                            action = Some(Action::SaveDescription(selected_id));
                        }

                        ui.add_space(10.0);
                        ui.strong(self.localizer.t("detail.jots"));
                        let jot = ui.add(
                            egui::TextEdit::singleline(&mut self.quick_jot_text)
                                .desired_width(f32::INFINITY)
                                .hint_text(self.localizer.t("detail.jot_hint")),
                        );
                        let submitted =
                            jot.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if submitted {
                            // Keep focus so several jots can be chained without re-clicking.
                            jot.request_focus();
                            action = Some(Action::QuickJot(selected_id));
                        }
                        if !self.task_notes.is_empty() {
                            egui::ScrollArea::vertical()
                                .id_salt("task_jots")
                                .max_height(260.0)
                                .auto_shrink([false, true])
                                .show(ui, |ui| {
                                    for note in &self.task_notes {
                                        egui::Frame::group(ui.style()).show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            if self.editing_jot == Some(note.id()) {
                                                let edit = ui.add(
                                                    egui::TextEdit::singleline(
                                                        &mut self.editing_jot_text,
                                                    )
                                                    .desired_width(f32::INFINITY),
                                                );
                                                let saved = edit.lost_focus()
                                                    && ui
                                                        .input(|i| i.key_pressed(egui::Key::Enter));
                                                ui.horizontal(|ui| {
                                                    if saved
                                                        || ui
                                                            .small_button(
                                                                self.localizer.t("detail.save"),
                                                            )
                                                            .clicked()
                                                    {
                                                        action =
                                                            Some(Action::SaveJotEdit(note.id()));
                                                    }
                                                    if ui
                                                        .small_button(
                                                            self.localizer.t("detail.cancel"),
                                                        )
                                                        .clicked()
                                                    {
                                                        action = Some(Action::CancelJotEdit);
                                                    }
                                                });
                                            } else {
                                                CommonMarkViewer::new().show(
                                                    ui,
                                                    &mut self.md_cache,
                                                    note.body_markdown(),
                                                );
                                                ui.horizontal(|ui| {
                                                    let mut stamp = relative_time(
                                                        &self.localizer,
                                                        note.created_at(),
                                                        now,
                                                    );
                                                    if note.updated_at() != note.created_at() {
                                                        stamp.push(' ');
                                                        stamp.push_str(
                                                            self.localizer
                                                                .t("detail.edited_marker"),
                                                        );
                                                    }
                                                    ui.weak(stamp);
                                                    if ui
                                                        .small_button(
                                                            self.localizer.t("detail.edit"),
                                                        )
                                                        .clicked()
                                                    {
                                                        action = Some(Action::EditJot(note.id()));
                                                    }
                                                    if ui
                                                        .small_button(
                                                            self.localizer.t("detail.delete"),
                                                        )
                                                        .clicked()
                                                    {
                                                        action = Some(Action::DeleteJot(note.id()));
                                                    }
                                                });
                                            }
                                        });
                                    }
                                });
                        }
                    });

                    ui.separator();

                    // Right column: the metadata sidebar, Trello-style.
                    ui.vertical(|ui| {
                        ui.set_width(220.0);
                        let button_size = egui::vec2(ui.available_width(), 26.0);

                        // Live timer for the active task.
                        if task.status() == TaskStatus::InProgress {
                            if let Some(session) = self.active_session.as_ref() {
                                ui.strong(format!(
                                    "{} {}",
                                    phase_label(&self.localizer, session.phase()),
                                    format_mmss(session.remaining_seconds(now))
                                ));
                                if ui
                                    .add_sized(
                                        button_size,
                                        egui::Button::new(advance_label(
                                            &self.localizer,
                                            session.phase(),
                                        )),
                                    )
                                    .clicked()
                                {
                                    action = Some(Action::Advance(selected_id));
                                }
                            } else if let Some(phase) = self.pending_phase
                                && ui
                                    .add_sized(
                                        button_size,
                                        egui::Button::new(self.localizer.t_args(
                                            "card.start_phase",
                                            &[(
                                                "phase",
                                                phase_label(&self.localizer, phase).to_owned(),
                                            )],
                                        )),
                                    )
                                    .clicked()
                            {
                                action = Some(Action::StartNext(selected_id));
                            }
                            ui.separator();
                        }

                        ui.strong(self.localizer.t("detail.section_details"));
                        ui.label(self.localizer.t_args(
                            "detail.status",
                            &[(
                                "status",
                                self.localizer.t(status_key(task.status())).to_owned(),
                            )],
                        ));
                        let completed = self.progress.get(&selected_id).copied().unwrap_or(0);
                        ui.label(
                            self.localizer
                                .t_args("detail.progress", &[("completed", completed.to_string())]),
                        );
                        ui.horizontal(|ui| {
                            ui.label(self.localizer.t("detail.estimate"));
                            ui.add(egui::DragValue::new(&mut self.detail_estimate).range(0..=999));
                            if ui.small_button(self.localizer.t("detail.set")).clicked() {
                                action = Some(Action::SetEstimate(selected_id));
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(self.localizer.t("detail.effort"));
                            if let Some(new_effort) =
                                rating_combo(ui, &self.localizer, "effort_combo", task.effort())
                            {
                                action = Some(Action::SetRatings(
                                    selected_id,
                                    new_effort,
                                    task.effect(),
                                ));
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(self.localizer.t("detail.effect"));
                            if let Some(new_effect) =
                                rating_combo(ui, &self.localizer, "effect_combo", task.effect())
                            {
                                action = Some(Action::SetRatings(
                                    selected_id,
                                    task.effort(),
                                    new_effect,
                                ));
                            }
                        });

                        ui.label(self.localizer.t("detail.start_date"));
                        ui.horizontal(|ui| {
                            let mut has_start = self.detail_start.is_some();
                            if ui.checkbox(&mut has_start, "").changed() {
                                self.detail_start = has_start.then(|| Local::now().date_naive());
                                action = Some(Action::SetSchedule(selected_id));
                            }
                            if let Some(current) = self.detail_start {
                                let mut picker_date = to_picker_date(current);
                                let response = ui.add(
                                    DatePickerButton::new(&mut picker_date)
                                        .id_salt("start_date_picker"),
                                );
                                if response.changed()
                                    && let Some(updated) = from_picker_date(picker_date)
                                    && updated != current
                                {
                                    self.detail_start = Some(updated);
                                    action = Some(Action::SetSchedule(selected_id));
                                }
                            }
                        });
                        ui.label(self.localizer.t("detail.due_date"));
                        ui.horizontal(|ui| {
                            let mut has_due = self.detail_due.is_some();
                            if ui.checkbox(&mut has_due, "").changed() {
                                self.detail_due = has_due.then(|| Local::now().date_naive());
                                action = Some(Action::SetSchedule(selected_id));
                            }
                            if let Some(current) = self.detail_due {
                                let mut picker_date = to_picker_date(current);
                                let response = ui.add(
                                    DatePickerButton::new(&mut picker_date)
                                        .id_salt("due_date_picker"),
                                );
                                if response.changed()
                                    && let Some(updated) = from_picker_date(picker_date)
                                    && updated != current
                                {
                                    self.detail_due = Some(updated);
                                    action = Some(Action::SetSchedule(selected_id));
                                }
                            }
                        });
                        if let Some(parent_id) = task.linked_from() {
                            let parent_title = self
                                .tasks
                                .iter()
                                .find(|candidate| candidate.id() == parent_id)
                                .map(|candidate| candidate.title().to_owned());
                            if let Some(title) = parent_title {
                                ui.label(self.localizer.t("detail.follow_up_of"));
                                if ui.link(title).clicked() {
                                    action = Some(Action::Open(parent_id));
                                }
                            }
                        }

                        ui.separator();
                        ui.strong(self.localizer.t("detail.section_actions"));
                        let sidebar_button = |ui: &mut egui::Ui, key: &str| {
                            ui.add_sized(button_size, egui::Button::new(self.localizer.t(key)))
                                .clicked()
                        };
                        match task.status() {
                            TaskStatus::Todo => {
                                if sidebar_button(ui, "card.start") {
                                    action = Some(Action::Start(selected_id));
                                }
                            }
                            TaskStatus::InProgress => {
                                if sidebar_button(ui, "card.pause") {
                                    action = Some(Action::Pause(selected_id));
                                }
                                if sidebar_button(ui, "card.complete") {
                                    action = Some(Action::CompleteTask(selected_id));
                                }
                                if sidebar_button(ui, "card.cancel") {
                                    action = Some(Action::Cancel(selected_id));
                                }
                            }
                            TaskStatus::Paused => {
                                if sidebar_button(ui, "card.resume") {
                                    action = Some(Action::Start(selected_id));
                                }
                                if sidebar_button(ui, "card.complete") {
                                    action = Some(Action::CompleteTask(selected_id));
                                }
                                if sidebar_button(ui, "card.cancel") {
                                    action = Some(Action::Cancel(selected_id));
                                }
                            }
                            TaskStatus::Done | TaskStatus::Cancelled => {}
                        }
                        if task.status().is_terminal()
                            && sidebar_button(ui, "detail.create_follow_up")
                        {
                            action = Some(Action::CreateFollowUp(selected_id));
                        }
                        if sidebar_button(ui, "detail.close") {
                            action = Some(Action::CloseDetail);
                        }
                    });
                });
            });

            if response.should_close() {
                action = Some(Action::CloseDetail);
            }
        }

        if let Some(draft) = self.settings.as_mut() {
            let ctx = ui.ctx().clone();
            let response = egui::Modal::new(egui::Id::new("settings")).show(&ctx, |ui| {
                ui.set_width(340.0);
                ui.heading(self.localizer.t("settings.title"));
                egui::Grid::new("settings_grid")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label(self.localizer.t("settings.focus_minutes"));
                        ui.add(egui::DragValue::new(&mut draft.focus_minutes).range(1..=180));
                        ui.end_row();
                        ui.label(self.localizer.t("settings.short_break_minutes"));
                        ui.add(egui::DragValue::new(&mut draft.short_break_minutes).range(1..=60));
                        ui.end_row();
                        ui.label(self.localizer.t("settings.long_break_minutes"));
                        ui.add(egui::DragValue::new(&mut draft.long_break_minutes).range(1..=120));
                        ui.end_row();
                        ui.label(self.localizer.t("settings.long_break_after"));
                        ui.add(egui::DragValue::new(&mut draft.long_break_after).range(1..=12));
                        ui.end_row();
                        ui.label(self.localizer.t("settings.auto_start_breaks"));
                        ui.checkbox(&mut draft.auto_start_break, "");
                        ui.end_row();
                        ui.label(self.localizer.t("settings.auto_start_focus"));
                        ui.checkbox(&mut draft.auto_start_focus, "");
                        ui.end_row();
                        ui.label(self.localizer.t("settings.leisure_per_pomo"));
                        ui.add(
                            egui::DragValue::new(&mut draft.leisure_minutes_per_pomo)
                                .range(0..=120),
                        );
                        ui.end_row();
                        ui.label(self.localizer.t("settings.theme"));
                        ui.horizontal(|ui| {
                            ui.selectable_value(
                                &mut draft.theme,
                                ThemeChoice::Light,
                                self.localizer.t("settings.theme_light"),
                            );
                            ui.selectable_value(
                                &mut draft.theme,
                                ThemeChoice::Dark,
                                self.localizer.t("settings.theme_dark"),
                            );
                        });
                        ui.end_row();
                        ui.label(self.localizer.t("settings.language"));
                        egui::ComboBox::from_id_salt("settings_language")
                            .selected_text(draft.language.native_name())
                            .show_ui(ui, |ui| {
                                for language in Language::ALL {
                                    ui.selectable_value(
                                        &mut draft.language,
                                        language,
                                        language.native_name(),
                                    );
                                }
                            });
                        ui.end_row();
                    });
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(self.localizer.t("settings.save")).clicked() {
                        action = Some(Action::SaveSettings);
                    }
                    if ui.button(self.localizer.t("settings.cancel")).clicked() {
                        action = Some(Action::CloseSettings);
                    }
                });
            });
            if response.should_close() {
                action = Some(Action::CloseSettings);
            }
        }

        if let Some(action) = action {
            self.handle(action, now);
        }

        // Keep the countdown live only while a focus session is running.
        if self.active_session.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
    }
}

/// Renders the read-only day-scale Gantt of open tasks that have a due date. Bars run
/// from each task's `bar_start` to its due date; a vertical accent line marks today,
/// and clicking a row opens the task's detail.
fn timeline_view(
    ui: &mut egui::Ui,
    localizer: &Localizer,
    tasks: &[Task],
    today: NaiveDate,
    action: &mut Option<Action>,
) {
    const LABEL_WIDTH: f32 = 180.0;
    const DAY_WIDTH: f32 = 26.0;
    const ROW_HEIGHT: f32 = 30.0;
    const HEADER_HEIGHT: f32 = 24.0;

    let mut rows: Vec<&Task> = tasks
        .iter()
        .filter(|task| !task.status().is_terminal() && task.due_date().is_some())
        .collect();
    if rows.is_empty() {
        ui.label(localizer.t("timeline.empty_hint"));
        return;
    }
    rows.sort_by_key(|task| (task.bar_start(), task.due_date()));

    // The visible date range: everything scheduled plus today, padded on both sides.
    let mut first = today;
    let mut last = today;
    for task in &rows {
        first = first.min(task.bar_start());
        if let Some(due) = task.due_date() {
            last = last.max(due).max(task.bar_start());
        }
    }
    let first = first - chrono::Duration::days(2);
    let last = last + chrono::Duration::days(2);
    let day_count = (last - first).num_days().max(1);

    #[expect(
        clippy::cast_precision_loss,
        reason = "timeline spans are far below f32 precision limits"
    )]
    let days_f32 = |days: i64| -> f32 { days as f32 };
    let x_of = |rect: &egui::Rect, date: NaiveDate| -> f32 {
        rect.left() + LABEL_WIDTH + days_f32((date - first).num_days()) * DAY_WIDTH
    };

    egui::ScrollArea::both()
        .id_salt("timeline")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "row counts are far below f32 precision limits"
            )]
            let row_count = rows.len() as f32;
            let size = egui::vec2(
                LABEL_WIDTH + days_f32(day_count + 1) * DAY_WIDTH,
                HEADER_HEIGHT + row_count * ROW_HEIGHT,
            );
            let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let visuals = ui.visuals().clone();
            let painter = ui.painter().with_clip_rect(rect);
            let grid = visuals.widgets.noninteractive.bg_stroke.color;
            let text_color = visuals.widgets.noninteractive.fg_stroke.color;
            let accent = visuals.selection.stroke.color;

            // Weekly grid lines and date labels along the header.
            let mut day = first;
            while day <= last {
                let x = x_of(&rect, day);
                painter.line_segment(
                    [
                        egui::pos2(x, rect.top() + HEADER_HEIGHT),
                        egui::pos2(x, rect.bottom()),
                    ],
                    egui::Stroke::new(1.0, grid.gamma_multiply(0.5)),
                );
                painter.text(
                    egui::pos2(x + 2.0, rect.top() + HEADER_HEIGHT / 2.0),
                    egui::Align2::LEFT_CENTER,
                    day.format("%m-%d").to_string(),
                    egui::FontId::proportional(11.0),
                    text_color,
                );
                day += chrono::Duration::days(7);
            }
            // Today, as a vertical accent line.
            let today_x = x_of(&rect, today);
            painter.line_segment(
                [
                    egui::pos2(today_x, rect.top()),
                    egui::pos2(today_x, rect.bottom()),
                ],
                egui::Stroke::new(2.0, accent),
            );

            for (index, task) in rows.iter().enumerate() {
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "row indexes are far below f32 precision limits"
                )]
                let top = rect.top() + HEADER_HEIGHT + index as f32 * ROW_HEIGHT;
                let row_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), top),
                    egui::vec2(rect.width(), ROW_HEIGHT),
                );

                // Task title in the label column, clipped to its cell.
                let label_rect = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), top),
                    egui::vec2(LABEL_WIDTH - 8.0, ROW_HEIGHT),
                );
                painter.with_clip_rect(label_rect).text(
                    egui::pos2(rect.left() + 4.0, top + ROW_HEIGHT / 2.0),
                    egui::Align2::LEFT_CENTER,
                    task.title(),
                    egui::FontId::proportional(13.0),
                    text_color,
                );

                // The bar, tinted by the task's value score when rated.
                let due = task.due_date().unwrap_or_else(|| task.bar_start());
                let start = task.bar_start().min(due);
                let bar_rect = egui::Rect::from_min_max(
                    egui::pos2(x_of(&rect, start), top + 6.0),
                    egui::pos2(
                        x_of(&rect, due + chrono::Duration::days(1)),
                        top + ROW_HEIGHT - 6.0,
                    ),
                );
                let base = visuals.widgets.inactive.bg_fill;
                let fill = match task.value_score() {
                    Some(score) => theme::value_tint(base, score),
                    None => base,
                };
                let stroke_color = if task.is_overdue(today) {
                    visuals.error_fg_color
                } else {
                    accent
                };
                painter.rect_filled(bar_rect, egui::CornerRadius::same(4), fill);
                painter.rect_stroke(
                    bar_rect,
                    egui::CornerRadius::same(4),
                    egui::Stroke::new(1.5, stroke_color),
                    egui::StrokeKind::Inside,
                );

                // The whole row is clickable and opens the task's detail.
                let response = ui.interact(
                    row_rect,
                    egui::Id::new(("timeline_row", task.id().value())),
                    egui::Sense::click(),
                );
                if response.hovered() {
                    painter.rect_filled(row_rect, 0.0, accent.gamma_multiply(0.06));
                }
                if response.clicked() {
                    *action = Some(Action::Open(task.id()));
                }
            }
        });
}

/// Converts a domain date to the picker's jiff date. Any realistic calendar date
/// converts losslessly; out-of-range values fall back to jiff's default date.
fn to_picker_date(date: NaiveDate) -> jiff::civil::Date {
    use chrono::Datelike;
    match (
        i16::try_from(date.year()),
        i8::try_from(date.month()),
        i8::try_from(date.day()),
    ) {
        (Ok(year), Ok(month), Ok(day)) => {
            jiff::civil::Date::new(year, month, day).unwrap_or_default()
        }
        _ => jiff::civil::Date::default(),
    }
}

/// Converts a picker (jiff) date back to a domain date.
fn from_picker_date(date: jiff::civil::Date) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(
        i32::from(date.year()),
        u32::try_from(date.month()).ok()?,
        u32::try_from(date.day()).ok()?,
    )
}

/// A compact None/Low/Mid/High dropdown; returns the new value when the user changed it.
fn rating_combo(
    ui: &mut egui::Ui,
    localizer: &Localizer,
    id_salt: &str,
    current: Option<Rating>,
) -> Option<Option<Rating>> {
    let label_of = |value: Option<Rating>| match value {
        None => localizer.t("rating.none"),
        Some(Rating::Low) => localizer.t("rating.low"),
        Some(Rating::Mid) => localizer.t("rating.mid"),
        Some(Rating::High) => localizer.t("rating.high"),
    };
    let mut value = current;
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(label_of(value))
        .width(110.0)
        .show_ui(ui, |ui| {
            for option in [
                None,
                Some(Rating::Low),
                Some(Rating::Mid),
                Some(Rating::High),
            ] {
                ui.selectable_value(&mut value, option, label_of(option));
            }
        });
    (value != current).then_some(value)
}

/// Splits a comma-separated tag input into trimmed, non-empty tags.
fn parse_tags(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|tag| tag.trim().to_owned())
        .filter(|tag| !tag.is_empty())
        .collect()
}

/// Renders the notes screen (list + editor) from the app's disjoint note fields.
#[expect(
    clippy::too_many_arguments,
    reason = "renders the notes screen from disjoint app fields"
)]
fn notes_view(
    ui: &mut egui::Ui,
    localizer: &Localizer,
    notes: &[Note],
    selected_note: Option<NoteId>,
    note_backlinks: &[Note],
    note_search: &mut String,
    note_title: &mut String,
    note_tags_input: &mut String,
    note_body: &mut String,
    note_preview: &mut bool,
    md_cache: &mut CommonMarkCache,
    action: &mut Option<Action>,
) {
    ui.columns(2, |cols| {
        {
            let ui = &mut cols[0];
            ui.horizontal(|ui| {
                if ui.button(localizer.t("notes.new_note")).clicked() {
                    *action = Some(Action::NewNote);
                }
                if ui
                    .add(
                        egui::TextEdit::singleline(note_search)
                            .hint_text(localizer.t("notes.search_hint")),
                    )
                    .changed()
                {
                    *action = Some(Action::SearchNotes);
                }
            });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt("notes_list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for note in notes {
                        let mut label = String::new();
                        if note.pinned() {
                            label.push_str("* ");
                        }
                        label.push_str(note.title());
                        if note.archived() {
                            label.push(' ');
                            label.push_str(localizer.t("notes.archived_marker"));
                        }
                        if ui
                            .selectable_label(Some(note.id()) == selected_note, label)
                            .clicked()
                        {
                            *action = Some(Action::SelectNote(note.id()));
                        }
                    }
                });
        }
        {
            let ui = &mut cols[1];
            if let Some(note_id) = selected_note {
                let title_edit = ui.add(
                    egui::TextEdit::singleline(note_title)
                        .hint_text(localizer.t("notes.title_hint")),
                );
                let tags_edit = ui.add(
                    egui::TextEdit::singleline(note_tags_input)
                        .hint_text(localizer.t("notes.tags_hint")),
                );
                // Enter in the title or tags field saves the note.
                if (title_edit.lost_focus() || tags_edit.lost_focus())
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    *action = Some(Action::SaveNote(note_id));
                }
                ui.horizontal(|ui| {
                    if ui.button(localizer.t("notes.save")).clicked() {
                        *action = Some(Action::SaveNote(note_id));
                    }
                    ui.checkbox(note_preview, localizer.t("notes.preview"));
                    if let Some(note) = notes.iter().find(|note| note.id() == note_id) {
                        let pinned = note.pinned();
                        let pin_label =
                            localizer.t(if pinned { "notes.unpin" } else { "notes.pin" });
                        if ui.button(pin_label).clicked() {
                            *action = Some(Action::PinNote(note_id, !pinned));
                        }
                        let archived = note.archived();
                        let archive_label = localizer.t(if archived {
                            "notes.unarchive"
                        } else {
                            "notes.archive"
                        });
                        if ui.button(archive_label).clicked() {
                            *action = Some(Action::ArchiveNote(note_id, !archived));
                        }
                    }
                });
                if !note_backlinks.is_empty() {
                    ui.separator();
                    ui.label(localizer.t("notes.backlinks"));
                    for note in note_backlinks {
                        if ui.link(note.title()).clicked() {
                            *action = Some(Action::SelectNote(note.id()));
                        }
                    }
                }
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("note_editor")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if *note_preview {
                            CommonMarkViewer::new().show(ui, md_cache, note_body.as_str());
                        } else {
                            ui.add(
                                egui::TextEdit::multiline(note_body)
                                    .desired_rows(18)
                                    .desired_width(f32::INFINITY),
                            );
                        }
                    });
            } else {
                ui.label(localizer.t("notes.empty_hint"));
            }
        }
    });
}

/// Renders one task card and returns the action its buttons requested, if any.
#[expect(
    clippy::too_many_arguments,
    reason = "renders a card from the app's disjoint fields"
)]
fn card_ui(
    ui: &mut egui::Ui,
    localizer: &Localizer,
    task: &Task,
    completed: u32,
    active_session: Option<&PomodoroSession>,
    pending: Option<TimerPhase>,
    flash: Option<f32>,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> Option<Action> {
    let mut action = None;
    // Rated cards get a background blended toward their value score's hue
    // (green = quick win, red = money pit); unrated cards keep the plain frame.
    let mut frame = egui::Frame::group(ui.style());
    if let Some(score) = task.value_score() {
        frame = frame.fill(theme::value_tint(ui.visuals().panel_fill, score));
    }
    let inner = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.strong(task.title());
        ui.label(localizer.t_args(
            "card.pomo_progress",
            &[
                ("completed", completed.to_string()),
                ("estimated", task.estimated_pomos().to_string()),
            ],
        ));
        if let Some(due) = task.due_date() {
            let text =
                localizer.t_args("card.due", &[("date", due.format("%Y-%m-%d").to_string())]);
            if task.is_overdue(today) {
                ui.colored_label(ui.visuals().error_fg_color, text);
            } else {
                ui.weak(text);
            }
        }

        match task.status() {
            TaskStatus::Todo => {
                if ui.button(localizer.t("card.start")).clicked() {
                    action = Some(Action::Start(task.id()));
                }
            }
            TaskStatus::InProgress => {
                if let Some(session) = active_session {
                    ui.label(format!(
                        "{} {}",
                        phase_label(localizer, session.phase()),
                        format_mmss(session.remaining_seconds(now))
                    ));
                    if ui
                        .button(advance_label(localizer, session.phase()))
                        .clicked()
                    {
                        action = Some(Action::Advance(task.id()));
                    }
                } else if let Some(phase) = pending
                    && ui
                        .button(localizer.t_args(
                            "card.start_phase",
                            &[("phase", phase_label(localizer, phase).to_owned())],
                        ))
                        .clicked()
                {
                    action = Some(Action::StartNext(task.id()));
                }
                ui.horizontal(|ui| {
                    if ui.button(localizer.t("card.pause")).clicked() {
                        action = Some(Action::Pause(task.id()));
                    }
                    if ui.button(localizer.t("card.complete")).clicked() {
                        action = Some(Action::CompleteTask(task.id()));
                    }
                    if ui.button(localizer.t("card.cancel")).clicked() {
                        action = Some(Action::Cancel(task.id()));
                    }
                });
            }
            TaskStatus::Paused => {
                ui.horizontal(|ui| {
                    if ui.button(localizer.t("card.resume")).clicked() {
                        action = Some(Action::Start(task.id()));
                    }
                    if ui.button(localizer.t("card.complete")).clicked() {
                        action = Some(Action::CompleteTask(task.id()));
                    }
                    if ui.button(localizer.t("card.cancel")).clicked() {
                        action = Some(Action::Cancel(task.id()));
                    }
                });
            }
            TaskStatus::Done => {
                ui.label(localizer.t("card.done_marker"));
            }
            TaskStatus::Cancelled => {}
        }
    });

    // The whole card senses both: a quick click opens the detail, while moving the
    // pointer past egui's drag threshold starts a drag instead (they are exclusive).
    let card = inner
        .response
        .interact(egui::Sense::click_and_drag())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if action.is_none() {
        if card.drag_started() {
            action = Some(Action::BeginDrag(task.id()));
        } else if card.clicked() {
            action = Some(Action::Open(task.id()));
        }
    }

    let corner = egui::CornerRadius::same(6);
    let accent = ui.visuals().selection.stroke.color;
    // Hover lift: an accent glow that eases in and out.
    let lift = ui.ctx().animate_bool(card.id.with("lift"), card.hovered());
    if lift > 0.0 {
        ui.painter().rect_stroke(
            card.rect,
            corner,
            egui::Stroke::new(1.0 + lift, accent.gamma_multiply(0.5 * lift)),
            egui::StrokeKind::Outside,
        );
    }
    // Pomo-complete flash: a warm highlight fading out over the card.
    if let Some(progress) = flash
        && progress < 1.0
    {
        ui.painter().rect_filled(
            card.rect,
            corner,
            accent.gamma_multiply(0.25 * (1.0 - progress)),
        );
    }
    action
}

/// Maps dropping task `dropped` onto the `target` column to the action that performs the
/// matching transition, or `None` if the move is not a valid transition.
fn resolve_drop(target: TaskStatus, dropped: TaskId, tasks: &[Task]) -> Option<Action> {
    let current = tasks.iter().find(|task| task.id() == dropped)?.status();
    if !current.can_transition_to(target) {
        return None;
    }
    match target {
        TaskStatus::InProgress => Some(Action::Start(dropped)),
        TaskStatus::Paused => Some(Action::Pause(dropped)),
        TaskStatus::Done => Some(Action::CompleteTask(dropped)),
        TaskStatus::Todo | TaskStatus::Cancelled => None,
    }
}

/// Human-readable name of a timer phase.
fn phase_label(localizer: &Localizer, phase: TimerPhase) -> &str {
    localizer.t(match phase {
        TimerPhase::Focus => "phase.focus",
        TimerPhase::ShortBreak => "phase.short_break",
        TimerPhase::LongBreak => "phase.long_break",
    })
}

/// Label for the button that ends the current phase early.
fn advance_label(localizer: &Localizer, phase: TimerPhase) -> &str {
    localizer.t(if phase.is_effort() {
        "card.complete_pomo"
    } else {
        "card.skip_break"
    })
}

/// The localized, user-facing form of a service error.
fn error_message(localizer: &Localizer, error: &crate::app::Error) -> String {
    use crate::app::Error;
    match error {
        Error::TaskNotFound => localizer.t("error.task_not_found").to_owned(),
        Error::NoteNotFound => localizer.t("error.note_not_found").to_owned(),
        Error::TaskNotActive => localizer.t("error.task_not_active").to_owned(),
        Error::NoRunningSession => localizer.t("error.no_running_session").to_owned(),
        Error::RewardOverflow => localizer.t("error.reward_overflow").to_owned(),
        Error::Transition(_) => localizer.t("error.invalid_transition").to_owned(),
        Error::Schedule(_) => localizer.t("error.invalid_schedule").to_owned(),
        Error::Repository(inner) => {
            localizer.t_args("error.storage", &[("message", inner.to_string())])
        }
    }
}

/// Formats how long ago `then` was, relative to `now` ("just now", "5 min ago", …).
fn relative_time(localizer: &Localizer, then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let seconds = (now - then).num_seconds();
    if seconds < 60 {
        localizer.t("time.just_now").to_owned()
    } else if seconds < 3600 {
        localizer.t_args("time.minutes_ago", &[("n", (seconds / 60).to_string())])
    } else if seconds < 86_400 {
        localizer.t_args("time.hours_ago", &[("n", (seconds / 3600).to_string())])
    } else if seconds < 2 * 86_400 {
        localizer.t("time.yesterday").to_owned()
    } else if seconds < 7 * 86_400 {
        localizer.t_args("time.days_ago", &[("n", (seconds / 86_400).to_string())])
    } else {
        then.format("%Y-%m-%d").to_string()
    }
}

/// Formats a (possibly negative) second count as `MM:SS`, clamped at zero.
fn format_mmss(seconds: i64) -> String {
    let clamped = seconds.max(0);
    format!("{:02}:{:02}", clamped / 60, clamped % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_with(id: i64, status: TaskStatus) -> Task {
        let created_at = DateTime::from_timestamp(0, 0).expect("valid timestamp");
        Task::from_fields(
            TaskId::new(id),
            "t".to_owned(),
            String::new(),
            status,
            1,
            None,
            None,
            None,
            None,
            None,
            created_at,
            None,
        )
    }

    #[test]
    fn format_mmss_pads_and_clamps() {
        assert_eq!(format_mmss(1500), "25:00");
        assert_eq!(format_mmss(65), "01:05");
        assert_eq!(format_mmss(0), "00:00");
        assert_eq!(format_mmss(-5), "00:00");
    }

    #[test]
    fn relative_time_scales_with_age() {
        let localizer = Localizer::new(Language::English);
        let now = DateTime::from_timestamp(1_000_000, 0).expect("valid timestamp");
        let at = |seconds_ago: i64| {
            DateTime::from_timestamp(1_000_000 - seconds_ago, 0).expect("valid timestamp")
        };
        assert_eq!(relative_time(&localizer, at(5), now), "just now");
        assert_eq!(relative_time(&localizer, at(300), now), "5 min ago");
        assert_eq!(relative_time(&localizer, at(2 * 3600), now), "2 h ago");
        assert_eq!(relative_time(&localizer, at(30 * 3600), now), "yesterday");
        assert_eq!(relative_time(&localizer, at(3 * 86_400), now), "3 days ago");
        // Older than a week falls back to the date.
        assert_eq!(
            relative_time(&localizer, at(30 * 86_400), now),
            at(30 * 86_400).format("%Y-%m-%d").to_string()
        );
    }

    #[test]
    fn resolve_drop_maps_valid_moves() {
        let tasks = [
            task_with(1, TaskStatus::Todo),
            task_with(2, TaskStatus::InProgress),
            task_with(3, TaskStatus::Paused),
        ];
        assert!(matches!(
            resolve_drop(TaskStatus::InProgress, TaskId::new(1), &tasks),
            Some(Action::Start(_))
        ));
        assert!(matches!(
            resolve_drop(TaskStatus::Paused, TaskId::new(2), &tasks),
            Some(Action::Pause(_))
        ));
        assert!(matches!(
            resolve_drop(TaskStatus::Done, TaskId::new(2), &tasks),
            Some(Action::CompleteTask(_))
        ));
        assert!(matches!(
            resolve_drop(TaskStatus::InProgress, TaskId::new(3), &tasks),
            Some(Action::Start(_))
        ));
    }

    #[test]
    fn resolve_drop_rejects_invalid_moves() {
        let tasks = [
            task_with(1, TaskStatus::Todo),
            task_with(2, TaskStatus::InProgress),
        ];
        // Todo cannot jump straight to Done.
        assert!(resolve_drop(TaskStatus::Done, TaskId::new(1), &tasks).is_none());
        // Nothing transitions back to Todo.
        assert!(resolve_drop(TaskStatus::Todo, TaskId::new(2), &tasks).is_none());
        // Dropping onto the same column is a no-op (no self-transition).
        assert!(resolve_drop(TaskStatus::Todo, TaskId::new(1), &tasks).is_none());
        // Unknown task id.
        assert!(resolve_drop(TaskStatus::Done, TaskId::new(99), &tasks).is_none());
    }
}
