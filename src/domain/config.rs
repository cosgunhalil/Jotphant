//! Aggregate application configuration.
//!
//! Pure value type mirroring the sections of `config.toml`. Loading/saving it (and the
//! `serde` mapping) is the storage layer's job; the domain only holds the values.

use crate::domain::pomodoro::PomodoroConfig;

/// The user's color-theme preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    /// Warm light theme (cream background, amber accents).
    #[default]
    Light,
    /// Warm dark theme (charcoal background, amber accents).
    Dark,
}

/// All configurable settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    pomodoro: PomodoroConfig,
    leisure_minutes_per_pomo: u32,
    theme: ThemeChoice,
}

impl AppConfig {
    /// Builds an application config.
    #[must_use]
    pub fn new(
        pomodoro: PomodoroConfig,
        leisure_minutes_per_pomo: u32,
        theme: ThemeChoice,
    ) -> Self {
        Self {
            pomodoro,
            leisure_minutes_per_pomo,
            theme,
        }
    }

    /// The user's color-theme preference.
    #[must_use]
    pub fn theme(&self) -> ThemeChoice {
        self.theme
    }

    /// The Pomodoro cycle configuration.
    #[must_use]
    pub fn pomodoro(&self) -> PomodoroConfig {
        self.pomodoro
    }

    /// Leisure minutes earned per banked pomo (the reward exchange rate).
    #[must_use]
    pub fn leisure_minutes_per_pomo(&self) -> u32 {
        self.leisure_minutes_per_pomo
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::new(PomodoroConfig::default(), 5, ThemeChoice::default())
    }
}
