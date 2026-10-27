//! Réglages modifiables depuis l'interface, avec leurs bornes.

use std::ops::RangeInclusive;

use pswd_infra::{Config, Theme};
use serde::{Deserialize, Serialize};

use crate::error::UiError;

/// 0 désactive le verrouillage par inactivité ; au plus une journée.
const AUTO_LOCK_MINUTES: RangeInclusive<u32> = 0..=1_440;
const CLIPBOARD_CLEAR_SECONDS: RangeInclusive<u32> = 5..=300;
/// 0 verrouille dès que l'application passe en arrière-plan ; au plus une heure.
const BACKGROUND_LOCK_SECONDS: RangeInclusive<u32> = 0..=3_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    auto_lock_minutes: u32,
    clipboard_clear_seconds: u32,
    background_lock_seconds: u32,
    theme: Theme,
}

impl From<&Config> for Settings {
    fn from(config: &Config) -> Self {
        Self {
            auto_lock_minutes: config.auto_lock_minutes,
            clipboard_clear_seconds: config.clipboard_clear_seconds,
            background_lock_seconds: config.background_lock_seconds,
            theme: config.theme,
        }
    }
}

impl Settings {
    /// # Errors
    ///
    /// `invalidSettings` si une valeur sort de ses bornes.
    pub fn check(&self) -> Result<(), UiError> {
        let valid = AUTO_LOCK_MINUTES.contains(&self.auto_lock_minutes)
            && CLIPBOARD_CLEAR_SECONDS.contains(&self.clipboard_clear_seconds)
            && BACKGROUND_LOCK_SECONDS.contains(&self.background_lock_seconds);
        if valid {
            Ok(())
        } else {
            Err(UiError::code("invalidSettings"))
        }
    }

    pub fn apply_to(self, config: &mut Config) {
        config.auto_lock_minutes = self.auto_lock_minutes;
        config.clipboard_clear_seconds = self.clipboard_clear_seconds;
        config.background_lock_seconds = self.background_lock_seconds;
        config.theme = self.theme;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn settings(auto_lock: u32, clipboard: u32, background: u32) -> Settings {
        Settings {
            auto_lock_minutes: auto_lock,
            clipboard_clear_seconds: clipboard,
            background_lock_seconds: background,
            theme: Theme::Dark,
        }
    }

    #[test]
    fn settings_come_from_and_go_to_the_configuration() {
        let config = Config::default();
        let read = Settings::from(&config);
        assert_eq!(
            serde_json::to_value(read).unwrap(),
            json!({
                "autoLockMinutes": 10,
                "clipboardClearSeconds": 20,
                "backgroundLockSeconds": 60,
                "theme": "auto",
            })
        );
        let mut config = Config::default();
        settings(3, 30, 0).apply_to(&mut config);
        assert_eq!(Settings::from(&config), settings(3, 30, 0));
    }

    #[test]
    fn values_must_stay_within_their_bounds() {
        for valid in [settings(0, 5, 0), settings(1_440, 300, 3_600)] {
            assert_eq!(valid.check(), Ok(()));
        }
        for invalid in [
            settings(1_441, 20, 60),
            settings(10, 4, 60),
            settings(10, 301, 60),
            settings(10, 20, 3_601),
        ] {
            assert_eq!(
                invalid.check(),
                Err(UiError::code("invalidSettings")),
                "{invalid:?}"
            );
        }
    }
}
