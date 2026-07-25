//! egui presentation layer.
//!
//! Talks only to [`crate::app`] services (none exist yet). For now it renders an empty,
//! titled window; real screens arrive in piece 0.5.

use eframe::egui;

/// The root eframe application shell.
pub struct JotphantApp;

impl JotphantApp {
    /// Creates the application shell.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for JotphantApp {
    fn default() -> Self {
        Self::new()
    }
}

impl eframe::App for JotphantApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.heading("Jotphant");
        });
    }
}
