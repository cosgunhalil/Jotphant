//! egui presentation layer — a Trello-style board.
//!
//! Talks only to [`crate::app::TaskService`]. Cards are grouped into status columns
//! (`Todo · In Progress · Paused · Done`; `Cancelled` is hidden). The view caches what it
//! displays and refreshes after each action rather than querying storage every frame.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use eframe::egui;

use crate::app::TaskService;
use crate::domain::ids::TaskId;
use crate::domain::repository::{
    BankRepository, SessionRepository, TaskRepository, Transactional,
};
use crate::domain::session::PomodoroSession;
use crate::domain::task::{Task, TaskStatus};

/// The columns shown on the board, in order. `Cancelled` is intentionally omitted.
const COLUMNS: [(TaskStatus, &str); 4] = [
    (TaskStatus::Todo, "Todo"),
    (TaskStatus::InProgress, "In Progress"),
    (TaskStatus::Paused, "Paused"),
    (TaskStatus::Done, "Done"),
];

/// A user action captured during rendering and applied afterwards, so rendering never
/// borrows `self` while a mutating service call runs.
enum Action {
    Create,
    Start(TaskId),
    Pause(TaskId),
    Cancel(TaskId),
    CompletePomo(TaskId),
    CompleteTask(TaskId),
    Open(TaskId),
    SaveDescription(TaskId),
    CloseDetail,
}

/// The root eframe application.
pub struct JotphantApp<S> {
    service: TaskService<S>,
    new_title: String,
    new_estimate: u32,
    tasks: Vec<Task>,
    progress: HashMap<TaskId, u32>,
    balance: i64,
    active_task: Option<Task>,
    active_session: Option<PomodoroSession>,
    status: Option<String>,
    selected: Option<TaskId>,
    detail_description: String,
}

impl<S> JotphantApp<S>
where
    S: TaskRepository + SessionRepository + BankRepository + Transactional,
{
    /// Builds the app over an injected service and loads the initial state.
    pub fn new(service: TaskService<S>) -> Self {
        let mut app = Self {
            service,
            new_title: String::new(),
            new_estimate: 1,
            tasks: Vec::new(),
            progress: HashMap::new(),
            balance: 0,
            active_task: None,
            active_session: None,
            status: None,
            selected: None,
            detail_description: String::new(),
        };
        app.refresh();
        app
    }

    /// Reloads cached tasks, per-task progress, balance, and the active task/session.
    fn refresh(&mut self) {
        match self.service.list_tasks() {
            Ok(tasks) => self.tasks = tasks,
            Err(error) => self.status = Some(error.to_string()),
        }
        let mut progress = HashMap::new();
        for task in &self.tasks {
            match self.service.completed_pomos(task.id()) {
                Ok(count) => {
                    progress.insert(task.id(), count);
                }
                Err(error) => self.status = Some(error.to_string()),
            }
        }
        self.progress = progress;
        match self.service.bank_balance() {
            Ok(balance) => self.balance = balance,
            Err(error) => self.status = Some(error.to_string()),
        }
        self.active_task = match self.service.active_task() {
            Ok(task) => task,
            Err(error) => {
                self.status = Some(error.to_string());
                None
            }
        };
        let active_id = self.active_task.as_ref().map(Task::id);
        self.active_session = match active_id {
            Some(id) => match self.service.active_focus_session(id) {
                Ok(session) => session,
                Err(error) => {
                    self.status = Some(error.to_string());
                    None
                }
            },
            None => None,
        };
    }

    /// Auto-completes the active focus pomo once its countdown reaches zero.
    fn tick(&mut self, now: DateTime<Utc>) {
        let expired = match (&self.active_task, &self.active_session) {
            (Some(task), Some(session)) if remaining_seconds(session, now) <= 0 => Some(task.id()),
            _ => None,
        };
        if let Some(task_id) = expired {
            if let Err(error) = self.service.complete_active_pomo(task_id, now) {
                self.status = Some(error.to_string());
            }
            self.refresh();
        }
    }

    /// Applies a captured action and refreshes.
    fn handle(&mut self, action: Action, now: DateTime<Utc>) {
        let result = match action {
            Action::Open(id) => {
                self.detail_description = self
                    .tasks
                    .iter()
                    .find(|task| task.id() == id)
                    .map(|task| task.description().to_owned())
                    .unwrap_or_default();
                self.selected = Some(id);
                return;
            }
            Action::CloseDetail => {
                self.selected = None;
                return;
            }
            Action::Create => {
                let title = self.new_title.trim().to_owned();
                if title.is_empty() {
                    self.status = Some("task title is required".to_owned());
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
            Action::CompletePomo(id) => self.service.complete_active_pomo(id, now),
            Action::CompleteTask(id) => self.service.complete_task(id, now).map(|_| ()),
            Action::SaveDescription(id) => self
                .service
                .set_task_description(id, self.detail_description.clone())
                .map(|_| ()),
        };
        self.status = result.err().map(|error| error.to_string());
        self.refresh();
    }
}

impl<S> eframe::App for JotphantApp<S>
where
    S: TaskRepository + SessionRepository + BankRepository + Transactional,
{
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = Utc::now();
        self.tick(now);

        let mut action: Option<Action> = None;
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Jotphant");
                ui.separator();
                ui.label(format!("Bank: {} pomos", self.balance));
            });
            if let Some(message) = &self.status {
                ui.colored_label(egui::Color32::RED, message);
            }
            ui.separator();

            ui.columns(COLUMNS.len(), |columns| {
                for (index, (status, title)) in COLUMNS.iter().enumerate() {
                    let ui = &mut columns[index];
                    let count = self.tasks.iter().filter(|t| t.status() == *status).count();
                    ui.strong(format!("{title}  ({count})"));
                    ui.separator();

                    egui::ScrollArea::vertical()
                        .id_salt(*title)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let (_, dropped) = ui.dnd_drop_zone::<TaskId, ()>(
                                egui::Frame::default(),
                                |ui| {
                                    ui.set_min_height(60.0);
                                    let mut any = false;
                                    for task in self.tasks.iter().filter(|t| t.status() == *status) {
                                        any = true;
                                        let completed =
                                            self.progress.get(&task.id()).copied().unwrap_or(0);
                                        let session = if Some(task.id())
                                            == self.active_task.as_ref().map(Task::id)
                                        {
                                            self.active_session.as_ref()
                                        } else {
                                            None
                                        };
                                        if let Some(card_action) =
                                            card_ui(ui, task, completed, session, now)
                                        {
                                            action = Some(card_action);
                                        }
                                    }
                                    if !any {
                                        ui.weak("Drop here");
                                    }
                                },
                            );
                            if let Some(dropped_id) = dropped
                                && let Some(dropped_action) =
                                    resolve_drop(*status, *dropped_id, &self.tasks)
                            {
                                action = Some(dropped_action);
                            }

                            if *status == TaskStatus::Todo {
                                ui.separator();
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.new_title)
                                        .hint_text("New task title"),
                                );
                                ui.horizontal(|ui| {
                                    ui.label("est");
                                    ui.add(egui::DragValue::new(&mut self.new_estimate).range(0..=999));
                                    if ui.button("Add").clicked() {
                                        action = Some(Action::Create);
                                    }
                                });
                            }
                        });
                }
            });
        });

        if let Some(selected_id) = self.selected {
            let ctx = ui.ctx().clone();
            let response = egui::Modal::new(egui::Id::new("task_detail")).show(&ctx, |ui| {
                ui.set_width(420.0);
                let Some(task) = self.tasks.iter().find(|task| task.id() == selected_id) else {
                    action = Some(Action::CloseDetail);
                    return;
                };

                ui.heading(task.title());
                ui.label(format!("Status: {:?}", task.status()));
                let completed = self.progress.get(&selected_id).copied().unwrap_or(0);
                ui.label(format!("Progress: {}/{} pomos", completed, task.estimated_pomos()));

                if task.status() == TaskStatus::InProgress
                    && let Some(session) = self.active_session.as_ref()
                {
                    ui.label(format!("⏱ {}", format_mmss(remaining_seconds(session, now))));
                    if ui.button("Complete pomo").clicked() {
                        action = Some(Action::CompletePomo(selected_id));
                    }
                }

                ui.separator();
                ui.label("Description");
                ui.add(
                    egui::TextEdit::multiline(&mut self.detail_description)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
                if ui.button("Save description").clicked() {
                    action = Some(Action::SaveDescription(selected_id));
                }

                ui.separator();
                ui.horizontal(|ui| {
                    match task.status() {
                        TaskStatus::Todo => {
                            if ui.button("Start").clicked() {
                                action = Some(Action::Start(selected_id));
                            }
                        }
                        TaskStatus::InProgress => {
                            if ui.button("Pause").clicked() {
                                action = Some(Action::Pause(selected_id));
                            }
                            if ui.button("Complete").clicked() {
                                action = Some(Action::CompleteTask(selected_id));
                            }
                            if ui.button("Cancel").clicked() {
                                action = Some(Action::Cancel(selected_id));
                            }
                        }
                        TaskStatus::Paused => {
                            if ui.button("Resume").clicked() {
                                action = Some(Action::Start(selected_id));
                            }
                            if ui.button("Complete").clicked() {
                                action = Some(Action::CompleteTask(selected_id));
                            }
                            if ui.button("Cancel").clicked() {
                                action = Some(Action::Cancel(selected_id));
                            }
                        }
                        TaskStatus::Done | TaskStatus::Cancelled => {}
                    }
                    if ui.button("Close").clicked() {
                        action = Some(Action::CloseDetail);
                    }
                });
            });

            if response.should_close() {
                action = Some(Action::CloseDetail);
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

/// Renders one task card and returns the action its buttons requested, if any.
fn card_ui(
    ui: &mut egui::Ui,
    task: &Task,
    completed: u32,
    active_session: Option<&PomodoroSession>,
    now: DateTime<Utc>,
) -> Option<Action> {
    let mut action = None;
    let inner = egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            // Only this handle is draggable, so clicking the rest of the card can open it.
            let handle_id = egui::Id::new(("card_drag", task.id().value()));
            ui.dnd_drag_source(handle_id, task.id(), |ui| {
                ui.label(":::");
            })
            .response
            .on_hover_cursor(egui::CursorIcon::Grab)
            .on_hover_text("Drag to move");
            ui.strong(task.title());
        });
        ui.label(format!("{completed}/{} pomos", task.estimated_pomos()));

        match task.status() {
            TaskStatus::Todo => {
                if ui.button("Start").clicked() {
                    action = Some(Action::Start(task.id()));
                }
            }
            TaskStatus::InProgress => {
                if let Some(session) = active_session {
                    ui.label(format!("⏱ {}", format_mmss(remaining_seconds(session, now))));
                    if ui.button("Complete pomo").clicked() {
                        action = Some(Action::CompletePomo(task.id()));
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Pause").clicked() {
                        action = Some(Action::Pause(task.id()));
                    }
                    if ui.button("Complete").clicked() {
                        action = Some(Action::CompleteTask(task.id()));
                    }
                    if ui.button("Cancel").clicked() {
                        action = Some(Action::Cancel(task.id()));
                    }
                });
            }
            TaskStatus::Paused => {
                ui.horizontal(|ui| {
                    if ui.button("Resume").clicked() {
                        action = Some(Action::Start(task.id()));
                    }
                    if ui.button("Complete").clicked() {
                        action = Some(Action::CompleteTask(task.id()));
                    }
                    if ui.button("Cancel").clicked() {
                        action = Some(Action::Cancel(task.id()));
                    }
                });
            }
            TaskStatus::Done => {
                ui.label("done");
            }
            TaskStatus::Cancelled => {}
        }
    });

    // Clicking anywhere on the card body (not a button or the handle) opens its detail.
    let card = inner
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if card.clicked() && action.is_none() {
        action = Some(Action::Open(task.id()));
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

/// Seconds left in a focus session (may be negative once elapsed).
fn remaining_seconds(session: &PomodoroSession, now: DateTime<Utc>) -> i64 {
    let elapsed = (now - session.started_at()).num_seconds();
    i64::from(session.configured_duration_seconds()) - elapsed
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
