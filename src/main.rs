//! Binary entry point and composition root.
//!
//! Constructs the application shell and hands it to eframe. As the app grows, this is
//! the single place that wires configuration, storage, and services together (see
//! `CODING_STANDARDS.md` §5).

use jotphant::ui::JotphantApp;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Jotphant",
        native_options,
        Box::new(|_cc| Ok(Box::new(JotphantApp::new()))),
    )
}
