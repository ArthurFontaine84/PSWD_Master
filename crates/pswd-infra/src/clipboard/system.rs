use std::time::Duration;

use pswd_core::Secret;
use pswd_core::ports::{PortError, SecureClipboard};
use zeroize::Zeroizing;

use super::{ClipboardAccess, TimedClipboard};

/// Presse-papiers du système.
pub struct SystemClipboard(arboard::Clipboard);

// Exclu de la couverture : le presse-papiers réel est absent des environnements de test sans
// session graphique ; la logique est testée avec un presse-papiers factice.
#[cfg_attr(coverage_nightly, coverage(off))]
impl SystemClipboard {
    /// # Errors
    ///
    /// Si le presse-papiers du système est inaccessible.
    pub fn new() -> Result<Self, PortError> {
        arboard::Clipboard::new()
            .map(Self)
            .map_err(|error| clipboard_error(&error))
    }
}

// Exclu de la couverture : même raison que `SystemClipboard::new`.
#[cfg_attr(coverage_nightly, coverage(off))]
impl ClipboardAccess for SystemClipboard {
    fn set_secret_text(&mut self, text: &str) -> Result<(), PortError> {
        let set = self.0.set();
        // Les trois marqueurs : certains gestionnaires de presse-papiers n'en lisent qu'un.
        #[cfg(target_os = "windows")]
        let set = {
            use arboard::SetExtWindows;
            set.exclude_from_monitoring()
                .exclude_from_cloud()
                .exclude_from_history()
        };
        #[cfg(target_os = "macos")]
        let set = arboard::SetExtApple::exclude_from_history(set);
        #[cfg(all(unix, not(target_os = "macos")))]
        let set = arboard::SetExtLinux::exclude_from_history(set);
        set.text(text).map_err(|error| clipboard_error(&error))
    }

    fn text(&mut self) -> Option<Zeroizing<String>> {
        self.0.get_text().ok().map(Zeroizing::new)
    }

    fn clear(&mut self) -> Result<(), PortError> {
        self.0.clear().map_err(|error| clipboard_error(&error))
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn clipboard_error(error: &arboard::Error) -> PortError {
    PortError::new(format!("presse-papiers : {error}"))
}

/// Presse-papiers du système ouvert à la première copie, effacé automatiquement : une machine
/// sans presse-papiers (serveur) peut utiliser tout le reste.
#[derive(Default)]
pub struct LazyClipboard(Option<TimedClipboard<SystemClipboard>>);

// Exclu de la couverture : même raison que `SystemClipboard::new`.
#[cfg_attr(coverage_nightly, coverage(off))]
impl SecureClipboard for LazyClipboard {
    fn copy_secret(&mut self, value: &Secret, ttl: Duration) -> Result<(), PortError> {
        if self.0.is_none() {
            self.0 = Some(TimedClipboard::new(SystemClipboard::new()?));
        }
        match &mut self.0 {
            Some(clipboard) => clipboard.copy_secret(value, ttl),
            None => Ok(()),
        }
    }
}
