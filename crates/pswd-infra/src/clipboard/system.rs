use pswd_core::ports::PortError;
use zeroize::Zeroizing;

use super::ClipboardAccess;

/// Presse-papiers du système.
pub struct SystemClipboard(arboard::Clipboard);

// Exclu de la couverture : le presse-papiers réel est absent des environnements de test (CI
// sans session graphique) ; la logique est testée avec un presse-papiers factice.
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
