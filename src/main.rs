//! Binary entry point and composition root.
//!
//! Opens the SQLite store, wires it into the task service, and hands the service to the
//! egui app. This is the single place that constructs concrete infrastructure (see
//! `CODING_STANDARDS.md` §5).

use std::error::Error;
use std::path::{Path, PathBuf};

use jotphant::app::TaskService;
use jotphant::domain::AppConfig;
use jotphant::notifier::DesktopNotifier;
use jotphant::storage::{SqliteStore, config};
use jotphant::ui::JotphantApp;

const CONFIG_PATH: &str = "config.toml";

fn main() -> Result<(), Box<dyn Error>> {
    let config = config::load_or_create(Path::new(CONFIG_PATH))?;
    let store = SqliteStore::open(Path::new("jotphant.db"))?;
    let service = TaskService::new(store, config);

    // Injected so the settings screen can persist config without the UI touching storage.
    let config_path = PathBuf::from(CONFIG_PATH);
    let save_config = Box::new(move |config: &AppConfig| {
        config::save(&config_path, config).map_err(|error| error.to_string())
    });

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Jotphant",
        native_options,
        Box::new(|_cc| {
            let notifier = Box::new(DesktopNotifier::new());
            Ok(Box::new(JotphantApp::new(service, save_config, notifier)))
        }),
    )?;
    Ok(())
}
