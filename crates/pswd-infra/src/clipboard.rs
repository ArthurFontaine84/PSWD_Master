use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use pswd_core::Secret;
use pswd_core::ports::{PortError, SecureClipboard};
use zeroize::Zeroizing;

/// Accès brut au presse-papiers.
pub trait ClipboardAccess: Send + 'static {
    /// Écrit `text` en l'excluant, si le système le permet, de l'historique du presse-papiers.
    ///
    /// # Errors
    ///
    /// Si le presse-papiers est inaccessible.
    fn set_secret_text(&mut self, text: &str) -> Result<(), PortError>;

    /// `None` si le presse-papiers est vide, illisible ou ne contient pas de texte.
    fn text(&mut self) -> Option<Zeroizing<String>>;

    /// # Errors
    ///
    /// Si le presse-papiers est inaccessible.
    fn clear(&mut self) -> Result<(), PortError>;
}

struct State<A> {
    access: A,
    /// Numéro de la dernière copie : seul son minuteur peut effacer.
    generation: u64,
    copied: Option<Secret>,
}

impl<A: ClipboardAccess> State<A> {
    /// N'efface que si le presse-papiers contient encore le secret : ce que l'utilisateur a
    /// copié depuis n'est jamais touché.
    fn clear_if_still_copied(&mut self) {
        if let Some(copied) = self.copied.take()
            && self
                .access
                .text()
                .is_some_and(|text| text.as_str() == copied.expose())
        {
            // Au mieux : un échec ici ne peut être signalé à personne.
            let _ = self.access.clear();
        }
    }
}

/// Presse-papiers qui efface un secret après un délai, et au plus tard à sa fermeture.
pub struct TimedClipboard<A: ClipboardAccess> {
    state: Arc<Mutex<State<A>>>,
}

impl<A: ClipboardAccess> TimedClipboard<A> {
    #[must_use]
    pub fn new(access: A) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                access,
                generation: 0,
                copied: None,
            })),
        }
    }
}

impl<A: ClipboardAccess> SecureClipboard for TimedClipboard<A> {
    fn copy_secret(&mut self, value: &Secret, ttl: Duration) -> Result<(), PortError> {
        let generation = {
            let mut state = lock(&self.state);
            state.access.set_secret_text(value.expose())?;
            state.generation += 1;
            state.copied = Some(value.clone());
            state.generation
        };
        let state = Arc::clone(&self.state);
        thread::spawn(move || {
            thread::sleep(ttl);
            let mut state = lock(&state);
            if state.generation == generation {
                state.clear_if_still_copied();
            }
        });
        Ok(())
    }
}

impl<A: ClipboardAccess> Drop for TimedClipboard<A> {
    fn drop(&mut self) {
        lock(&self.state).clear_if_still_copied();
    }
}

/// Un minuteur qui a paniqué ne doit pas bloquer le presse-papiers : l'état reste cohérent.
fn lock<A>(state: &Mutex<State<A>>) -> MutexGuard<'_, State<A>> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

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

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[derive(Default)]
    struct FakeData {
        content: Option<String>,
        broken: bool,
    }

    /// Presse-papiers en mémoire, partagé avec le test pour l'inspecter.
    #[derive(Clone, Default)]
    struct Fake(Arc<Mutex<FakeData>>);

    impl Fake {
        fn data(&self) -> MutexGuard<'_, FakeData> {
            self.0.lock().unwrap()
        }

        fn content(&self) -> Option<String> {
            self.data().content.clone()
        }
    }

    impl ClipboardAccess for Fake {
        fn set_secret_text(&mut self, text: &str) -> Result<(), PortError> {
            let mut data = self.data();
            if data.broken {
                return Err(PortError::new("presse-papiers occupé"));
            }
            data.content = Some(text.to_owned());
            Ok(())
        }

        fn text(&mut self) -> Option<Zeroizing<String>> {
            self.data().content.clone().map(Zeroizing::new)
        }

        fn clear(&mut self) -> Result<(), PortError> {
            let mut data = self.data();
            if data.broken {
                return Err(PortError::new("presse-papiers occupé"));
            }
            data.content = None;
            Ok(())
        }
    }

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    /// Attend que `condition` devienne vraie ; échoue au-delà de deux secondes.
    fn wait_until(condition: impl Fn() -> bool) {
        let start = Instant::now();
        while !condition() {
            assert!(start.elapsed() < Duration::from_secs(2), "délai dépassé");
            thread::sleep(Duration::from_millis(10));
        }
    }

    const SHORT: Duration = Duration::from_millis(30);
    const LONG: Duration = Duration::from_secs(60);

    #[test]
    fn copied_secret_is_cleared_after_the_delay() {
        let fake = Fake::default();
        let mut clipboard = TimedClipboard::new(fake.clone());
        clipboard.copy_secret(&secret("hunter2"), SHORT).unwrap();
        assert_eq!(fake.content().as_deref(), Some("hunter2"));
        wait_until(|| fake.content().is_none());
    }

    #[test]
    fn content_copied_since_is_never_cleared() {
        let fake = Fake::default();
        let mut clipboard = TimedClipboard::new(fake.clone());
        clipboard.copy_secret(&secret("hunter2"), SHORT).unwrap();
        fake.data().content = Some("texte de l'utilisateur".to_owned());
        thread::sleep(SHORT * 5);
        assert_eq!(fake.content().as_deref(), Some("texte de l'utilisateur"));
        drop(clipboard);
        assert_eq!(fake.content().as_deref(), Some("texte de l'utilisateur"));
    }

    #[test]
    fn only_the_latest_copy_timer_clears() {
        let fake = Fake::default();
        let mut clipboard = TimedClipboard::new(fake.clone());
        clipboard.copy_secret(&secret("premier"), SHORT).unwrap();
        clipboard.copy_secret(&secret("second"), LONG).unwrap();
        thread::sleep(SHORT * 5);
        assert_eq!(fake.content().as_deref(), Some("second"));
    }

    #[test]
    fn closing_clears_the_secret_immediately() {
        let fake = Fake::default();
        let mut clipboard = TimedClipboard::new(fake.clone());
        clipboard.copy_secret(&secret("hunter2"), LONG).unwrap();
        drop(clipboard);
        assert_eq!(fake.content(), None);
    }

    #[test]
    fn closing_without_copy_leaves_the_clipboard_alone() {
        let fake = Fake::default();
        fake.data().content = Some("autre".to_owned());
        drop(TimedClipboard::new(fake.clone()));
        assert_eq!(fake.content().as_deref(), Some("autre"));
    }

    #[test]
    fn failed_copy_is_reported_and_nothing_is_tracked() {
        let fake = Fake::default();
        fake.data().broken = true;
        let mut clipboard = TimedClipboard::new(fake.clone());
        assert_eq!(
            clipboard.copy_secret(&secret("hunter2"), SHORT),
            Err(PortError::new("presse-papiers occupé"))
        );
        assert_eq!(fake.content(), None);
    }

    #[test]
    fn failed_clear_is_ignored() {
        let fake = Fake::default();
        let mut clipboard = TimedClipboard::new(fake.clone());
        clipboard.copy_secret(&secret("hunter2"), LONG).unwrap();
        fake.data().broken = true;
        drop(clipboard);
        assert_eq!(fake.content().as_deref(), Some("hunter2"));
    }
}
