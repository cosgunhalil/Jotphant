//! Aggregate application configuration.
//!
//! Pure value type mirroring the sections of `config.toml`. Loading/saving it (and the
//! `serde` mapping) is the storage layer's job; the domain only holds the values.

use crate::domain::pomodoro::PomodoroConfig;

/// All configurable settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    pomodoro: PomodoroConfig,
    leisure_minutes_per_pomo: u32,
}

impl AppConfig {
    /// Builds an application config.
    #[must_use]
    pub fn new(pomodoro: PomodoroConfig, leisure_minutes_per_pomo: u32) -> Self {
        Self {
            pomodoro,
            leisure_minutes_per_pomo,
        }
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
        Self::new(PomodoroConfig::default(), 5)
    }
}
