use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use pswd_app::{IdleLock, VaultSession, Vaults, live_count, search, tag_counts};
use pswd_core::crypto::KdfParams;
use pswd_core::ports::SecureClipboard;
use pswd_core::{Credentials, Entry, Secret, Timestamp, Vault};
use serde::Serialize;
use uuid::Uuid;

use crate::entries::{EntryDetail, EntrySummary, TagCount};
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

/// Presse-papiers de la plateforme, qui s'efface tout seul.
pub type Clipboard = Box<dyn SecureClipboard + Send>;

/// État partagé par toutes les commandes : un seul coffre ouvert à la fois, gardé en Rust.
pub struct AppState {
    pub vaults: Vaults,
    kdf: KdfParams,
    clock: fn() -> Timestamp,
    /// Délai de verrouillage imposé ; `None` : celui de la configuration.
    auto_lock: Option<Duration>,
    on_lock: LockNotifier,
    clipboard: Mutex<Clipboard>,
    open: Mutex<Option<OpenVault>>,
}

impl AppState {
    pub fn new(
        vaults: Vaults,
        clipboard: Clipboard,
        on_lock: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::with_settings(
            vaults,
            clipboard,
            KdfParams::DEFAULT,
            pswd_app::now,
            None,
            on_lock,
        )
    }

    pub fn with_settings(
        vaults: Vaults,
        clipboard: Clipboard,
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
            clipboard: Mutex::new(clipboard),
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

    /// Entrées non supprimées qui correspondent à `text` et à `tag`, triées par titre.
    pub fn entries(&self, text: &str, tag: Option<&str>) -> Result<Vec<EntrySummary>, UiError> {
        self.with_vault(|vault| {
            search(vault, text, tag)
                .into_iter()
                .map(EntrySummary::from)
                .collect()
        })
    }

    pub fn entry(&self, id: &str) -> Result<EntryDetail, UiError> {
        let id = parse_id(id)?;
        self.with_vault(|vault| find(vault, id).map(EntryDetail::from))?
    }

    /// Tags des entrées non supprimées, avec leur nombre d'entrées.
    pub fn tags(&self) -> Result<Vec<TagCount>, UiError> {
        self.with_vault(|vault| {
            tag_counts(vault)
                .into_iter()
                .map(|(tag, count)| TagCount::new(tag, count))
                .collect()
        })
    }

    /// Le mot de passe d'une entrée, pour l'afficher à la demande de l'utilisateur.
    pub fn reveal_password(&self, id: &str) -> Result<Secret, UiError> {
        let id = parse_id(id)?;
        self.with_vault(|vault| password_of(vault, id))?
    }

    /// Copie le mot de passe sans le transmettre à l'interface ; renvoie le délai d'effacement.
    pub fn copy_password(&self, id: &str) -> Result<Duration, UiError> {
        let ttl = self.vaults.config()?.clipboard_clear();
        let password = self.reveal_password(id)?;
        self.clipboard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .copy_secret(&password, ttl)
            .map_err(|error| UiError::new("clipboard", error.to_string()))?;
        Ok(ttl)
    }

    /// Lit le coffre ouvert, et repousse son verrouillage.
    fn with_vault<T>(&self, read: impl FnOnce(&Vault) -> T) -> Result<T, UiError> {
        let open = self.open();
        let open = open.as_ref().ok_or_else(UiError::vault_locked)?;
        open.lock
            .with(|session| read(session.vault()))
            .ok_or_else(UiError::vault_locked)
    }

    fn open(&self) -> MutexGuard<'_, Option<OpenVault>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Un identifiant illisible désigne une entrée qui n'existe pas.
fn parse_id(id: &str) -> Result<Uuid, UiError> {
    Uuid::parse_str(id).map_err(|_| UiError::new("entryNotFound", id))
}

/// Entrée non supprimée.
fn find(vault: &Vault, id: Uuid) -> Result<&Entry, UiError> {
    vault
        .get(id)
        .filter(|entry| !entry.is_deleted())
        .ok_or_else(|| UiError::new("entryNotFound", id.to_string()))
}

fn password_of(vault: &Vault, id: Uuid) -> Result<Secret, UiError> {
    let content = find(vault, id)?.content();
    content
        .password
        .clone()
        .ok_or_else(|| UiError::new("noPassword", content.title.clone()))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    use pswd_app::AppError;
    use pswd_core::ports::PortError;
    use pswd_core::{EntryContent, EntryKind, Tags};
    use pswd_infra::Locations;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    /// Copies reçues par un presse-papiers factice.
    type Copies = Arc<Mutex<Vec<(String, Duration)>>>;

    #[derive(Default)]
    struct FakeClipboard {
        copies: Copies,
        broken: bool,
    }

    impl SecureClipboard for FakeClipboard {
        fn copy_secret(&mut self, value: &Secret, ttl: Duration) -> Result<(), PortError> {
            if self.broken {
                return Err(PortError::new("presse-papiers indisponible"));
            }
            self.copies
                .lock()
                .unwrap()
                .push((value.expose().to_owned(), ttl));
            Ok(())
        }
    }

    fn vaults_in(dir: &TempDir) -> Vaults {
        Vaults::new(Locations::new(
            &dir.path().join("config"),
            &dir.path().join("data"),
        ))
    }

    /// État dans un dossier temporaire, et le nombre de verrouillages par inactivité signalés.
    fn state_with(
        dir: &TempDir,
        auto_lock: Option<Duration>,
        clipboard: FakeClipboard,
    ) -> (AppState, Arc<AtomicUsize>) {
        let kdf = KdfParams::new(8 * 1024, 1, 1).unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let notified = Arc::clone(&count);
        let state = AppState::with_settings(
            vaults_in(dir),
            Box::new(clipboard),
            kdf,
            || Timestamp::from_millis(1_000),
            auto_lock,
            move || {
                notified.fetch_add(1, Ordering::SeqCst);
            },
        );
        (state, count)
    }

    fn state_in(dir: &TempDir, auto_lock: Option<Duration>) -> (AppState, Arc<AtomicUsize>) {
        state_with(dir, auto_lock, FakeClipboard::default())
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
        let calls = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&calls);
        let clipboard = Box::new(FakeClipboard::default());
        let state = AppState::new(vaults_in(&dir), clipboard, move || {
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

    fn login(title: &str, username: &str, password: &str, tags: &[&str]) -> EntryContent {
        EntryContent {
            username: Some(username.to_owned()),
            password: Some(secret(password)),
            tags: Tags::new(tags.iter().copied()),
            ..EntryContent::new(EntryKind::Login, title.to_owned())
        }
    }

    /// Coffre `perso` ouvert : GitHub et banque (identifiants), Wi-Fi (note), et une entrée
    /// supprimée. Renvoie les identifiants de GitHub, de la note et de l'entrée supprimée.
    fn open_with_entries(state: &AppState) -> (String, String, String) {
        let (mut session, _) = state
            .vaults
            .create(
                "perso",
                &secret("maître"),
                state.kdf,
                Timestamp::from_millis(0),
            )
            .unwrap();
        let ids = session
            .edit(|vault| {
                let at = Timestamp::from_millis(1);
                let github = vault.add(login("GitHub", "arthur", "hunter2", &["dev", "perso"]), at);
                vault.add(login("banque", "a.fontaine", "1234", &["perso"]), at);
                let note = vault.add(EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()), at);
                let deleted = vault.add(login("Ancienne", "arthur", "x", &[]), at);
                vault.delete(deleted, Timestamp::from_millis(2)).unwrap();
                (github.to_string(), note.to_string(), deleted.to_string())
            })
            .unwrap();
        drop(session);
        state.unlock("perso", &secret("maître")).unwrap();
        ids
    }

    fn titles(entries: &[EntrySummary]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| {
                serde_json::to_value(entry).unwrap()["title"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect()
    }

    #[test]
    fn reading_entries_needs_an_open_vault() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let id = Uuid::nil().to_string();
        let locked = Some(UiError::vault_locked());
        assert_eq!(state.entries("", None).err(), locked);
        assert_eq!(state.entry(&id).err(), locked);
        assert_eq!(state.tags().err(), locked);
        assert_eq!(state.reveal_password(&id).err(), locked);
        assert_eq!(state.copy_password(&id).err(), locked);
    }

    #[test]
    fn entries_are_searched_and_filtered_by_tag() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        open_with_entries(&state);
        assert_eq!(
            titles(&state.entries("", None).unwrap()),
            ["banque", "GitHub", "Wi-Fi"]
        );
        assert_eq!(titles(&state.entries("ARTHUR", None).unwrap()), ["GitHub"]);
        assert_eq!(
            titles(&state.entries("", Some("perso")).unwrap()),
            ["banque", "GitHub"]
        );
        assert_eq!(
            serde_json::to_value(state.tags().unwrap()).unwrap(),
            json!([{ "tag": "dev", "count": 1 }, { "tag": "perso", "count": 2 }])
        );
    }

    #[test]
    fn entry_detail_needs_a_live_entry() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (github, _, deleted) = open_with_entries(&state);
        let detail = serde_json::to_value(state.entry(&github).unwrap()).unwrap();
        assert_eq!(detail["title"], "GitHub");
        assert_eq!(
            state.entry(&deleted).err(),
            Some(UiError::new("entryNotFound", deleted.clone()))
        );
        assert_eq!(
            state.entry("pas-un-id").err(),
            Some(UiError::new("entryNotFound", "pas-un-id"))
        );
    }

    #[test]
    fn password_is_revealed_only_for_entries_that_have_one() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (github, note, _) = open_with_entries(&state);
        assert_eq!(state.reveal_password(&github).unwrap().expose(), "hunter2");
        assert_eq!(
            state.reveal_password(&note).err(),
            Some(UiError::new("noPassword", "Wi-Fi"))
        );
        assert_eq!(
            state.reveal_password("pas-un-id").err(),
            Some(UiError::new("entryNotFound", "pas-un-id"))
        );
    }

    #[test]
    fn copied_password_goes_to_the_clipboard_only() {
        let dir = TempDir::new().unwrap();
        let clipboard = FakeClipboard::default();
        let copies = Arc::clone(&clipboard.copies);
        let (state, _) = state_with(&dir, None, clipboard);
        let (github, note, _) = open_with_entries(&state);
        assert_eq!(state.copy_password(&github), Ok(Duration::from_secs(20)));
        assert_eq!(
            *copies.lock().unwrap(),
            [("hunter2".to_owned(), Duration::from_secs(20))]
        );
        assert_eq!(
            state.copy_password(&note).err(),
            Some(UiError::new("noPassword", "Wi-Fi"))
        );
        assert_eq!(copies.lock().unwrap().len(), 1);
    }

    #[test]
    fn clipboard_failure_is_reported() {
        let dir = TempDir::new().unwrap();
        let clipboard = FakeClipboard {
            broken: true,
            ..FakeClipboard::default()
        };
        let (state, _) = state_with(&dir, None, clipboard);
        let (github, _, _) = open_with_entries(&state);
        assert_eq!(
            state.copy_password(&github).err(),
            Some(UiError::new("clipboard", "presse-papiers indisponible"))
        );
    }

    #[test]
    fn reading_after_the_automatic_lock_is_refused() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, Some(Duration::from_millis(100)));
        open_with_entries(&state);
        thread::sleep(Duration::from_millis(400));
        assert_eq!(state.entries("", None).err(), Some(UiError::vault_locked()));
    }
}
