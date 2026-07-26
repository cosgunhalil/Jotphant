//! Loading and saving [`AppConfig`] as a TOML file.
//!
//! The on-disk format uses human-friendly **minutes**; the domain uses seconds, so this
//! module converts at the boundary. `serde` lives only on the private DTO types here,
//! never on the domain (see `CODING_STANDARDS.md` §2).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::domain::config::{AppConfig, ThemeChoice};
use crate::domain::pomodoro::PomodoroConfig;
use crate::domain::session::TimerPhase;

/// An error loading or saving the config file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Reading or writing the file failed.
    #[error("config i/o error: {0}")]
    Io(#[from] std::io::Error),
    /// The file was not valid TOML for our schema.
    #[error("config parse error: {0}")]
    Parse(#[from] toml::de::Error),
    /// Serializing the config to TOML failed.
    #[error("config serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct ConfigDto {
    pomodoro: PomodoroDto,
    rewards: RewardsDto,
    ui: UiDto,
}

#[derive(Debug, Serialize, Deserialize)]
struct UiDto {
    theme: ThemeDto,
}

impl Default for UiDto {
    fn default() -> Self {
        Self {
            theme: ThemeDto::Light,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ThemeDto {
    Light,
    Dark,
}

impl ThemeDto {
    fn into_domain(self) -> ThemeChoice {
        match self {
            Self::Light => ThemeChoice::Light,
            Self::Dark => ThemeChoice::Dark,
        }
    }

    fn from_domain(theme: ThemeChoice) -> Self {
        match theme {
            ThemeChoice::Light => Self::Light,
            ThemeChoice::Dark => Self::Dark,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PomodoroDto {
    focus_minutes: u32,
    short_break_minutes: u32,
    long_break_minutes: u32,
    long_break_after: u32,
    auto_start_break: bool,
    auto_start_focus: bool,
}

impl Default for PomodoroDto {
    fn default() -> Self {
        Self {
            focus_minutes: 25,
            short_break_minutes: 5,
            long_break_minutes: 15,
            long_break_after: 4,
            auto_start_break: true,
            auto_start_focus: true,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RewardsDto {
    leisure_minutes_per_pomo: u32,
}

impl Default for RewardsDto {
    fn default() -> Self {
        Self {
            leisure_minutes_per_pomo: 5,
        }
    }
}

impl ConfigDto {
    fn into_domain(self) -> AppConfig {
        AppConfig::new(
            PomodoroConfig::new(
                self.pomodoro.focus_minutes.saturating_mul(60),
                self.pomodoro.short_break_minutes.saturating_mul(60),
                self.pomodoro.long_break_minutes.saturating_mul(60),
                self.pomodoro.long_break_after,
                self.pomodoro.auto_start_break,
                self.pomodoro.auto_start_focus,
            ),
            self.rewards.leisure_minutes_per_pomo,
            self.ui.theme.into_domain(),
        )
    }

    fn from_domain(config: &AppConfig) -> Self {
        let pomodoro = config.pomodoro();
        Self {
            ui: UiDto {
                theme: ThemeDto::from_domain(config.theme()),
            },
            pomodoro: PomodoroDto {
                focus_minutes: pomodoro.duration_seconds(TimerPhase::Focus) / 60,
                short_break_minutes: pomodoro.duration_seconds(TimerPhase::ShortBreak) / 60,
                long_break_minutes: pomodoro.duration_seconds(TimerPhase::LongBreak) / 60,
                long_break_after: pomodoro.long_break_after(),
                auto_start_break: pomodoro.should_auto_start(TimerPhase::ShortBreak),
                auto_start_focus: pomodoro.should_auto_start(TimerPhase::Focus),
            },
            rewards: RewardsDto {
                leisure_minutes_per_pomo: config.leisure_minutes_per_pomo(),
            },
        }
    }
}

/// Parses config from a TOML string. Missing sections fall back to defaults.
///
/// # Errors
/// Returns [`ConfigError::Parse`] if the text is not valid TOML for the schema.
pub fn parse(text: &str) -> Result<AppConfig, ConfigError> {
    let dto: ConfigDto = toml::from_str(text)?;
    Ok(dto.into_domain())
}

/// Serializes config to a pretty TOML string.
///
/// # Errors
/// Returns [`ConfigError::Serialize`] if serialization fails.
pub fn to_toml(config: &AppConfig) -> Result<String, ConfigError> {
    let text = toml::to_string_pretty(&ConfigDto::from_domain(config))?;
    Ok(text)
}

/// Loads the config from `path`, creating it with defaults if it does not yet exist.
///
/// # Errors
/// Returns [`ConfigError`] on an I/O, parse, or serialize failure.
pub fn load_or_create(path: &Path) -> Result<AppConfig, ConfigError> {
    if path.exists() {
        let text = fs::read_to_string(path)?;
        parse(&text)
    } else {
        let config = AppConfig::default();
        fs::write(path, to_toml(&config)?)?;
        Ok(config)
    }
}

/// Saves the config to `path`, overwriting any existing file.
///
/// # Errors
/// Returns [`ConfigError`] on an I/O or serialize failure.
pub fn save(path: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    fs::write(path, to_toml(config)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_through_toml() {
        let config = AppConfig::default();
        let text = to_toml(&config).expect("serialize");
        let parsed = parse(&text).expect("parse");
        assert_eq!(parsed, config);
    }

    #[test]
    fn a_customized_config_round_trips() {
        let config = AppConfig::new(
            PomodoroConfig::new(50 * 60, 10 * 60, 20 * 60, 3, false, true),
            8,
            ThemeChoice::Dark,
        );
        let text = to_toml(&config).expect("serialize");
        let parsed = parse(&text).expect("parse");
        assert_eq!(parsed, config);
    }

    #[test]
    fn theme_defaults_to_light_when_ui_section_is_missing() {
        // A pre-theme config file has no [ui] section.
        let parsed = parse("[rewards]\nleisure_minutes_per_pomo = 9\n").expect("parse");
        assert_eq!(parsed.theme(), ThemeChoice::Light);
    }

    #[test]
    fn theme_parses_from_the_ui_section() {
        let parsed = parse("[ui]\ntheme = \"dark\"\n").expect("parse");
        assert_eq!(parsed.theme(), ThemeChoice::Dark);
    }

    #[test]
    fn minutes_in_the_file_become_seconds_in_the_domain() {
        let parsed = parse(
            "[pomodoro]\n\
             focus_minutes = 30\n\
             short_break_minutes = 6\n\
             long_break_minutes = 20\n\
             long_break_after = 2\n\
             auto_start_break = false\n\
             auto_start_focus = false\n\
             [rewards]\n\
             leisure_minutes_per_pomo = 7\n",
        )
        .expect("parse");
        assert_eq!(parsed.pomodoro().duration_seconds(TimerPhase::Focus), 1800);
        assert_eq!(parsed.leisure_minutes_per_pomo(), 7);
        assert!(!parsed.pomodoro().should_auto_start(TimerPhase::Focus));
    }

    #[test]
    fn missing_sections_fall_back_to_defaults() {
        // Only rewards provided; pomodoro should default.
        let parsed = parse("[rewards]\nleisure_minutes_per_pomo = 9\n").expect("parse");
        assert_eq!(parsed.pomodoro(), PomodoroConfig::default());
        assert_eq!(parsed.leisure_minutes_per_pomo(), 9);
    }
}
