use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use pswd_app::{IdleLock, VaultSession, Vaults, live_count};
use pswd_core::crypto::KdfParams;
use pswd_core::{Credentials, Secret, Timestamp};
use serde::Serialize;

use crate::error::UiError;

/// Coffre ouvert, tel que l'interface le voit : jamais de secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VaultInfo {
    pub name: String,
    pub entries: usize,
}

struct OpenVault {
    name: String,
    lock: IdleLock<VaultSession>,
}

/// Prévient l'interface qu'un coffre s'est verrouillé par inactivité.
type LockNotifier = Arc<dyn Fn() + Send + Sync>;

/// État partagé par toutes les commandes : un seul coffre ouvert à la fois, gardé en Rust.
pub struct AppState {
    pub vaults: Vaults,
    kdf: KdfParams,
    clock: fn() -> Timestamp,
    /// Délai de verrouillage imposé ; `None` : celui de la configuration.
    auto_lock: Option<Duration>,
    on_lock: LockNotifier,
    open: Mutex<Option<OpenVault>>,
}

impl AppState {
    pub fn new(vaults: Vaults, on_lock: impl Fn() + Send + Sync + 'static) -> Self {
        Self::with_settings(vaults, KdfParams::DEFAULT, pswd_app::now, None, on_lock)
    }

    pub fn with_settings(
        vaults: Vaults,
        kdf: KdfParams,
        clock: fn() -> Timestamp,
        auto_lock: Option<Duration>,
        on_lock: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            vaults,
            kdf,
            clock,
            auto_lock,
            on_lock: Arc::new(on_lock),
            open: Mutex::new(None),
        }
    }

    /// Crée le coffre et le garde ouvert. Renvoie aussi sa clé de récupération, à montrer une
    /// seule fois.
    pub fn create(&self, name: &str, password: &Secret) -> Result<(VaultInfo, Secret), UiError> {
        let timeout = self.auto_lock()?;
        let (session, recovery_key) =
            self.vaults
                .create(name, password, self.kdf, (self.clock)())?;
        let vault = self.keep_open(name, session, timeout);
        Ok((vault, recovery_key.encode()))
    }

    /// Ouvre le coffre ; un coffre déjà ouvert est d'abord verrouillé.
    pub fn unlock(&self, name: &str, password: &Secret) -> Result<VaultInfo, UiError> {
        let timeout = self.auto_lock()?;
        let session = self.vaults.open(name, Credentials::Password(password))?;
        Ok(self.keep_open(name, session, timeout))
    }

    fn auto_lock(&self) -> Result<Duration, UiError> {
        Ok(match self.auto_lock {
            Some(timeout) => timeout,
            None => self.vaults.config()?.auto_lock(),
        })
    }

    fn keep_open(&self, name: &str, session: VaultSession, timeout: Duration) -> VaultInfo {
        let vault = VaultInfo {
            name: name.to_owned(),
            entries: live_count(session.vault()),
        };
        let on_lock = Arc::clone(&self.on_lock);
        let lock = IdleLock::notifying(session, timeout, move || on_lock());
        let previous = self.open().replace(OpenVault {
            name: name.to_owned(),
            lock,
        });
        if let Some(previous) = previous {
            drop(previous.lock.close());
        }
        vault
    }

    /// Verrouille tout de suite : la clé du coffre est effacée de la mémoire.
    pub fn lock(&self) {
        if let Some(open) = self.open().take() {
            drop(open.lock.close());
        }
    }

    /// Le coffre ouvert, et repousse son verrouillage ; `None` s'il n'y en a pas, ou s'il s'est
    /// verrouillé par inactivité.
    pub fn current(&self) -> Option<VaultInfo> {
        let open = self.open();
        let open = open.as_ref()?;
        open.lock.with(|session| VaultInfo {
            name: open.name.clone(),
            entries: live_count(session.vault()),
        })
    }

    fn open(&self) -> MutexGuard<'_, Option<OpenVault>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    use pswd_app::AppError;
    use pswd_core::{EntryContent, EntryKind};
    use pswd_infra::Locations;
    use tempfile::TempDir;

    use super::*;

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    /// État dans un dossier temporaire, et le nombre de verrouillages par inactivité signalés.
    fn state_in(dir: &TempDir, auto_lock: Option<Duration>) -> (AppState, Arc<AtomicUsize>) {
        let vaults = Vaults::new(Locations::new(
            &dir.path().join("config"),
            &dir.path().join("data"),
        ));
        let kdf = KdfParams::new(8 * 1024, 1, 1).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let notified = Arc::clone(&count);
        let state = AppState::with_settings(
            vaults,
            kdf,
            || Timestamp::from_millis(1_000),
            auto_lock,
            move || {
                notified.fetch_add(1, Ordering::SeqCst);
            },
        );
        (state, count)
    }

    fn info(name: &str, entries: usize) -> VaultInfo {
        VaultInfo {
            name: name.to_owned(),
            entries,
        }
    }

    #[test]
    fn default_settings_create_a_usable_vault() {
        let dir = TempDir::new().unwrap();
        let vaults = Vaults::new(Locations::new(
            &dir.path().join("config"),
            &dir.path().join("data"),
        ));
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        let state = AppState::new(vaults, move || {
            counted.fetch_add(1, Ordering::SeqCst);
        });
        (state.on_lock)();
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        state.create("perso", &secret("maître")).unwrap();
        state.lock();
        assert_eq!(
            state.unlock("perso", &secret("maître")),
            Ok(info("perso", 0))
        );
    }

    #[test]
    fn created_vault_stays_open_and_gives_its_recovery_key() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (vault, recovery_key) = state.create("perso", &secret("maître")).unwrap();
        assert_eq!(vault, info("perso", 0));
        assert_eq!(recovery_key.expose().split('-').count(), 13);
        assert_eq!(state.current(), Some(info("perso", 0)));
    }

    #[test]
    fn creation_errors_are_reported() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        assert_eq!(
            state.create("perso", &secret("")).err(),
            Some(UiError::from(AppError::EmptyPassword))
        );
        assert_eq!(state.current(), None);
    }

    #[test]
    fn unlocking_needs_the_right_password() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (mut session, _) = state
            .vaults
            .create(
                "perso",
                &secret("maître"),
                state.kdf,
                Timestamp::from_millis(0),
            )
            .unwrap();
        session
            .edit(|vault| {
                vault.add(
                    EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
                    Timestamp::from_millis(1),
                );
            })
            .unwrap();
        drop(session);
        assert!(state.unlock("perso", &secret("autre")).is_err());
        assert_eq!(state.current(), None);
        assert_eq!(
            state.unlock("perso", &secret("maître")),
            Ok(info("perso", 1))
        );
        assert_eq!(state.current(), Some(info("perso", 1)));
    }

    #[test]
    fn only_one_vault_is_open_at_a_time() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state.create("perso", &secret("maître")).unwrap();
        state.create("travail", &secret("autre")).unwrap();
        assert_eq!(state.current(), Some(info("travail", 0)));
        state.unlock("perso", &secret("maître")).unwrap();
        assert_eq!(state.current(), Some(info("perso", 0)));
    }

    #[test]
    fn locking_closes_the_vault_without_notification() {
        let dir = TempDir::new().unwrap();
        let (state, count) = state_in(&dir, Some(Duration::from_millis(50)));
        state.create("perso", &secret("maître")).unwrap();
        state.lock();
        state.lock();
        thread::sleep(Duration::from_millis(200));
        assert_eq!(state.current(), None);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn idle_vault_locks_itself_and_says_so() {
        let dir = TempDir::new().unwrap();
        let (state, count) = state_in(&dir, Some(Duration::from_millis(30)));
        state.create("perso", &secret("maître")).unwrap();
        thread::sleep(Duration::from_millis(300));
        assert_eq!(state.current(), None);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn default_delay_comes_from_the_configuration() {
        let dir = TempDir::new().unwrap();
        let (state, count) = state_in(&dir, None);
        state.create("perso", &secret("maître")).unwrap();
        thread::sleep(Duration::from_millis(100));
        assert_eq!(state.current(), Some(info("perso", 0)));
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unreadable_configuration_keeps_the_vault_closed() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state.create("perso", &secret("maître")).unwrap();
        state.lock();
        let config = dir.path().join("config").join("config.toml");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}\nauto_lock_minutes = \"dix\"\n")).unwrap();
        assert!(state.unlock("perso", &secret("maître")).is_err());
        assert_eq!(state.current(), None);
    }
}
