//! Desktop notifications for Pomodoro phase transitions.
//!
//! Kept behind a trait so the UI stays decoupled from the platform crate; the composition
//! root injects the concrete implementation.

use crate::domain::session::TimerPhase;

/// Notifies the user of Pomodoro phase transitions.
pub trait Notifier {
    /// Called when the `ended` phase's timer elapsed and `started` (if any) began.
    fn on_phase_transition(&self, ended: TimerPhase, started: Option<TimerPhase>);
}

/// A [`Notifier`] that shows a desktop notification. On Windows the toast also plays the
/// system notification sound, satisfying the "notification + sound" requirement.
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
    fn on_phase_transition(&self, ended: TimerPhase, started: Option<TimerPhase>) {
        let (summary, body) = message(ended, started);
        // Best-effort: a failed notification must never disrupt the app.
        let _ = notify_rust::Notification::new()
            .summary(summary)
            .body(body)
            .show();
    }
}

/// The summary/body to show for a given transition.
fn message(ended: TimerPhase, started: Option<TimerPhase>) -> (&'static str, &'static str) {
    match ended {
        TimerPhase::Focus => match started {
            Some(TimerPhase::ShortBreak | TimerPhase::LongBreak) => {
                ("Focus complete", "Time for a break.")
            }
            _ => ("Focus complete", "Start your next break."),
        },
        TimerPhase::ShortBreak | TimerPhase::LongBreak => match started {
            Some(TimerPhase::Focus) => ("Break over", "Back to focus."),
            _ => ("Break over", "Start your next focus session."),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_transition_prompts_a_break() {
        assert_eq!(
            message(TimerPhase::Focus, Some(TimerPhase::ShortBreak)),
            ("Focus complete", "Time for a break.")
        );
    }

    #[test]
    fn break_transition_prompts_focus() {
        assert_eq!(
            message(TimerPhase::LongBreak, Some(TimerPhase::Focus)),
            ("Break over", "Back to focus.")
        );
    }

    #[test]
    fn transition_without_a_next_phase_prompts_a_manual_start() {
        assert_eq!(
            message(TimerPhase::Focus, None),
            ("Focus complete", "Start your next break.")
        );
    }
}
