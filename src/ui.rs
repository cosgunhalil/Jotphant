//! egui presentation layer.
//!
//! Talks only to [`crate::app::TaskService`]. It caches what it displays and refreshes
//! after each action (and when a pomo expires) rather than querying storage every frame.

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

/// A user action captured during rendering and applied after, so rendering never borrows
/// `self` while a mutating service call runs.
enum Action {
    Create,
    Start(TaskId),
    CompletePomo(TaskId),
    CompleteTask(TaskId),
}

/// The root eframe application.
pub struct JotphantApp<S> {
    service: TaskService<S>,
    new_title: String,
    new_estimate: u32,
    tasks: Vec<Task>,
    balance: i64,
    active_task: Option<Task>,
    active_session: Option<PomodoroSession>,
    status: Option<String>,
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
            balance: 0,
            active_task: None,
            active_session: None,
            status: None,
        };
        app.refresh();
        app
    }

    /// Reloads cached tasks, balance, and the active task/session from storage.
    fn refresh(&mut self) {
        match self.service.list_tasks() {
            Ok(tasks) => self.tasks = tasks,
            Err(error) => self.status = Some(error.to_string()),
        }
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
            Action::CompletePomo(id) => self.service.complete_active_pomo(id, now),
            Action::CompleteTask(id) => self.service.complete_task(id, now).map(|_| ()),
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
            ui.heading("Jotphant");
            ui.label(format!("Bank: {} pomos", self.balance));
            if let Some(message) = &self.status {
                ui.colored_label(egui::Color32::RED, message);
            }
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("New task:");
                ui.text_edit_singleline(&mut self.new_title);
                ui.label("estimate");
                ui.add(egui::DragValue::new(&mut self.new_estimate).range(0..=999));
                if ui.button("Add").clicked() {
                    action = Some(Action::Create);
                }
            });
            ui.separator();

            if let Some(task) = &self.active_task {
                ui.strong(format!("Active: {}", task.title()));
                if let Some(session) = &self.active_session {
                    ui.label(format!("Focus: {}", format_mmss(remaining_seconds(session, now))));
                    if ui.button("Complete pomo").clicked() {
                        action = Some(Action::CompletePomo(task.id()));
                    }
                } else {
                    ui.label("No running pomo");
                }
                if ui.button("Complete task").clicked() {
                    action = Some(Action::CompleteTask(task.id()));
                }
                ui.separator();
            }

            ui.heading("Tasks");
            for task in &self.tasks {
                ui.horizontal(|ui| {
                    ui.label(format!("{} — {:?}", task.title(), task.status()));
                    if matches!(task.status(), TaskStatus::Todo | TaskStatus::Paused)
                        && ui.button("Start").clicked()
                    {
                        action = Some(Action::Start(task.id()));
                    }
                });
            }
        });

        if let Some(action) = action {
            self.handle(action, now);
        }

        // Keep the countdown live only while a focus session is running.
        if self.active_session.is_some() {
            ui.ctx().request_repaint_after(Duration::from_millis(500));
        }
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

    #[test]
    fn format_mmss_pads_and_clamps() {
        assert_eq!(format_mmss(1500), "25:00");
        assert_eq!(format_mmss(65), "01:05");
        assert_eq!(format_mmss(0), "00:00");
        assert_eq!(format_mmss(-5), "00:00");
    }
}
