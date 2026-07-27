//! Desktop notifications.
//!
//! The trait receives already-localized text — deciding *what* to say (and in which
//! language) is the UI's job. Kept behind a trait so the UI stays decoupled from the
//! platform crate; the composition root injects the concrete implementation.

/// Shows notifications to the user.
pub trait Notifier {
    /// Shows a notification with the given summary and body.
    fn notify(&self, summary: &str, body: &str);
}

/// A [`Notifier`] backed by desktop toasts. On Windows the toast also plays the system
/// notification sound.
pub struct DesktopNotifier;

impl DesktopNotifier {
    /// Creates the notifier.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for DesktopNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl Notifier for DesktopNotifier {
    fn notify(&self, summary: &str, body: &str) {
        // Best-effort: a failed notification must never disrupt the app.
        let _ = notify_rust::Notification::new()
            .summary(summary)
            .body(body)
            .show();
    }
}
