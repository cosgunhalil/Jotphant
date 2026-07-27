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

/// The user's display-language preference.
///
/// Adding a language means adding a variant here plus a catalog file under
/// `locales/` — see `CONTRIBUTING.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    /// English (the reference catalog every other language is checked against).
    #[default]
    English,
    /// Turkish.
    Turkish,
    /// Spanish.
    Spanish,
    /// Azerbaijani.
    Azerbaijani,
}

impl Language {
    /// Every selectable language, in picker order.
    pub const ALL: [Self; 4] = [
        Self::English,
        Self::Turkish,
        Self::Spanish,
        Self::Azerbaijani,
    ];

    /// The two-letter language code, matching the catalog file name.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Turkish => "tr",
            Self::Spanish => "es",
            Self::Azerbaijani => "az",
        }
    }

    /// The language's name in itself, for the picker.
    #[must_use]
    pub fn native_name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Turkish => "Türkçe",
            Self::Spanish => "Español",
            Self::Azerbaijani => "Azərbaycanca",
        }
    }

    /// Matches a system locale tag (e.g. `en-US`, `tr-TR`) to a supported
    /// language by its primary subtag.
    #[must_use]
    pub fn from_locale(locale: &str) -> Option<Self> {
        let primary = locale.split(['-', '_']).next().unwrap_or(locale);
        Self::ALL
            .into_iter()
            .find(|language| language.code().eq_ignore_ascii_case(primary))
    }
}

/// All configurable settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppConfig {
    pomodoro: PomodoroConfig,
    leisure_minutes_per_pomo: u32,
    theme: ThemeChoice,
    language: Language,
}

impl AppConfig {
    /// Builds an application config.
    #[must_use]
    pub fn new(
        pomodoro: PomodoroConfig,
        leisure_minutes_per_pomo: u32,
        theme: ThemeChoice,
        language: Language,
    ) -> Self {
        Self {
            pomodoro,
            leisure_minutes_per_pomo,
            theme,
            language,
        }
    }

    /// The user's color-theme preference.
    #[must_use]
    pub fn theme(&self) -> ThemeChoice {
        self.theme
    }

    /// The user's display-language preference.
    #[must_use]
    pub fn language(&self) -> Language {
        self.language
    }

    /// Returns this config with a different language (used to seed the first-run
    /// default from the detected system locale).
    #[must_use]
    pub fn with_language(mut self, language: Language) -> Self {
        self.language = language;
        self
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
        Self::new(
            PomodoroConfig::default(),
            5,
            ThemeChoice::default(),
            Language::default(),
        )
    }
}
