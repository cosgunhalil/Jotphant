//! Binary entry point and composition root.
//!
//! Opens the SQLite store, wires it into the task service, and hands the service to the
//! egui app. This is the single place that constructs concrete infrastructure (see
//! `CODING_STANDARDS.md` §5).

use std::error::Error;
use std::path::Path;

use jotphant::app::TaskService;
use jotphant::storage::{SqliteStore, config};
use jotphant::ui::JotphantApp;

fn main() -> Result<(), Box<dyn Error>> {
    let config = config::load_or_create(Path::new("config.toml"))?;
    let store = SqliteStore::open(Path::new("jotphant.db"))?;
    let service = TaskService::new(store, config);

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Jotphant",
        native_options,
        Box::new(|_cc| Ok(Box::new(JotphantApp::new(service)))),
    )?;
    Ok(())
}
