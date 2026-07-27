//! Binary entry point and composition root.
//!
//! Resolves the platform data/config paths, opens the SQLite store, wires it into the
//! task service, and hands the service to the egui app. This is the single place that
//! constructs concrete infrastructure (see `CODING_STANDARDS.md` §5).

// In release builds, run as a GUI app without spawning a console window; debug builds
// keep the console for log output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use std::fs;
use std::path::PathBuf;

use directories::ProjectDirs;
use jotphant::app::TaskService;
use jotphant::domain::{AppConfig, Language};
use jotphant::notifier::DesktopNotifier;
use jotphant::storage::{SqliteStore, config};
use jotphant::ui::JotphantApp;

/// The database and config file paths, in the platform data/config directories (falling
/// back to the working directory if none can be resolved). Parent directories are created.
fn app_paths() -> Result<(PathBuf, PathBuf), Box<dyn Error>> {
    let (data_dir, config_dir) = match ProjectDirs::from("dev", "jotphant", "Jotphant") {
        Some(dirs) => (
            dirs.data_dir().to_path_buf(),
            dirs.config_dir().to_path_buf(),
        ),
        None => (PathBuf::from("."), PathBuf::from(".")),
    };
    fs::create_dir_all(&data_dir)?;
    fs::create_dir_all(&config_dir)?;
    Ok((data_dir.join("jotphant.db"), config_dir.join("config.toml")))
}

/// Extends egui's built-in fonts with a system font covering glyphs the defaults lack
/// (e.g. the Azerbaijani schwa "ə", which otherwise renders as a placeholder box).
/// Appended as the last fallback so the app's look is unchanged for covered glyphs.
/// Best-effort: silently skipped if the font file is absent.
fn install_font_fallback(ctx: &eframe::egui::Context) {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".to_owned());
    let path = PathBuf::from(windir).join("Fonts").join("segoeui.ttf");
    let Ok(bytes) = fs::read(&path) else {
        return;
    };
    let mut fonts = eframe::egui::FontDefinitions::default();
    fonts.font_data.insert(
        "system-fallback".to_owned(),
        std::sync::Arc::new(eframe::egui::FontData::from_owned(bytes)),
    );
    for family in [
        eframe::egui::FontFamily::Proportional,
        eframe::egui::FontFamily::Monospace,
    ] {
        fonts
            .families
            .entry(family)
            .or_default()
            .push("system-fallback".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn main() -> Result<(), Box<dyn Error>> {
    let (db_path, config_path) = app_paths()?;

    // On first run, default the language to the system locale when we support it.
    let detected_language = sys_locale::get_locale()
        .and_then(|tag| Language::from_locale(&tag))
        .unwrap_or_default();
    let first_run_default = AppConfig::default().with_language(detected_language);
    let config = config::load_or_create(&config_path, first_run_default)?;
    let store = SqliteStore::open(&db_path)?;
    let service = TaskService::new(store, config);

    // Injected so the settings screen can persist config without the UI touching storage.
    let save_config = Box::new(move |config: &AppConfig| {
        config::save(&config_path, config).map_err(|error| error.to_string())
    });

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Jotphant")
            .with_inner_size([1000.0, 700.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Jotphant",
        native_options,
        Box::new(|cc| {
            install_font_fallback(&cc.egui_ctx);
            let notifier = Box::new(DesktopNotifier::new());
            Ok(Box::new(JotphantApp::new(service, save_config, notifier)))
        }),
    )?;
    Ok(())
}
