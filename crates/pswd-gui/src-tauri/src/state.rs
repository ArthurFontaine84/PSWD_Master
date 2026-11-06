use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::Duration;

use pswd_app::{
    AppError, IdleLock, SyncOutcome, VaultSession, Vaults, add_entries, backup_file_name,
    check_master_password, live_count, search, tag_counts,
};
use pswd_core::crypto::KdfParams;
use pswd_core::ports::SecureClipboard;
use pswd_core::{
    Credentials, Entry, RecoveryKey, RecoveryKeyError, Secret, Timestamp, Vault, import_csv,
};
use serde::Serialize;
use uuid::Uuid;

use crate::editing::{EntryInput, HistoryItem, history};
use crate::entries::{EntryDetail, EntrySummary, TagCount};
use crate::error::UiError;
use crate::server::ServerTrouble;
use crate::settings::Settings;
use crate::sync::SyncReport;

/// Coffre ouvert, tel que l'interface le voit : jamais de secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VaultInfo {
    pub name: String,
    pub entries: usize,
}

struct OpenVault {
    name: String,
    /// Numéro de cette ouverture : une synchro ne rend sa copie qu'à l'ouverture d'où elle vient.
    opening: u64,
    /// Nombre de modifications faites depuis l'ouverture.
    edits: u64,
    lock: IdleLock<VaultSession>,
}

/// Ce que l'état signale à l'interface, depuis un autre fil.
pub trait Notifier: Send + Sync + 'static {
    /// Le coffre s'est verrouillé par inactivité.
    fn vault_locked(&self);
    /// Une synchro automatique du coffre ouvert est terminée.
    fn vault_synced(&self, report: &SyncReport);
}

type SharedVault = Arc<Mutex<Option<OpenVault>>>;

/// L'application est en arrière-plan depuis `since` ; le coffre se verrouille après `delay`.
struct Hidden {
    since: Timestamp,
    delay: Duration,
}

/// Le verrouillage en arrière-plan ne concerne que le mobile : sur le bureau, une fenêtre
/// réduite reste soumise au seul verrouillage par inactivité.
const BACKGROUND_LOCK: bool = cfg!(mobile);

/// Presse-papiers de la plateforme, qui s'efface tout seul.
pub type Clipboard = Box<dyn SecureClipboard + Send>;

/// État partagé par toutes les commandes : un seul coffre ouvert à la fois, gardé en Rust.
pub struct AppState {
    pub vaults: Vaults,
    kdf: KdfParams,
    pub(crate) clock: fn() -> Timestamp,
    /// Délai de verrouillage imposé ; `None` : celui de la configuration.
    auto_lock: Option<Duration>,
    background_lock: bool,
    notifier: Arc<dyn Notifier>,
    clipboard: Mutex<Clipboard>,
    open: SharedVault,
    openings: AtomicU64,
    /// Une seule synchro ou un seul échange avec le serveur à la fois : deux fils ne réservent
    /// jamais le même coffre et n'écrivent jamais la configuration en même temps.
    syncing: Arc<Mutex<()>>,
    hidden: Mutex<Option<Hidden>>,
    /// Échec du dernier contact avec le serveur ; `None` s'il a réussi, ou s'il n'y en a pas eu.
    trouble: Mutex<Option<ServerTrouble>>,
}

impl AppState {
    pub fn new(vaults: Vaults, clipboard: Clipboard, notifier: impl Notifier) -> Self {
        Self::with_settings(
            vaults,
            clipboard,
            KdfParams::DEFAULT,
            pswd_app::now,
            None,
            BACKGROUND_LOCK,
            notifier,
        )
    }

    pub fn with_settings(
        vaults: Vaults,
        clipboard: Clipboard,
        kdf: KdfParams,
        clock: fn() -> Timestamp,
        auto_lock: Option<Duration>,
        background_lock: bool,
        notifier: impl Notifier,
    ) -> Self {
        Self {
            vaults,
            kdf,
            clock,
            auto_lock,
            background_lock,
            notifier: Arc::new(notifier),
            clipboard: Mutex::new(clipboard),
            open: Arc::new(Mutex::new(None)),
            openings: AtomicU64::new(0),
            syncing: Arc::new(Mutex::new(())),
            hidden: Mutex::new(None),
            trouble: Mutex::new(None),
        }
    }

    /// Crée le coffre et le garde ouvert. Renvoie aussi sa clé de récupération, à montrer une
    /// seule fois.
    /// `shared` en fait un coffre commun, à partager avec d'autres comptes du serveur.
    pub fn create(
        &self,
        name: &str,
        password: &Secret,
        shared: bool,
    ) -> Result<(VaultInfo, Secret), UiError> {
        let timeout = self.auto_lock()?;
        let now = (self.clock)();
        let (session, recovery_key) = if shared {
            self.vaults.create_shared(name, password, self.kdf, now)?
        } else {
            self.vaults.create(name, password, self.kdf, now)?
        };
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

    /// Garde le coffre ouvert, puis le synchronise en arrière-plan.
    fn keep_open(&self, name: &str, session: VaultSession, timeout: Duration) -> VaultInfo {
        let vault = VaultInfo {
            name: name.to_owned(),
            entries: live_count(session.vault()),
        };
        let notifier = Arc::clone(&self.notifier);
        let lock = IdleLock::notifying(session, timeout, move || notifier.vault_locked());
        let previous = self.open().replace(OpenVault {
            name: name.to_owned(),
            opening: self.openings.fetch_add(1, Ordering::Relaxed),
            edits: 0,
            lock,
        });
        if let Some(previous) = previous {
            self.close(previous);
        }
        self.sync_later();
        vault
    }

    /// Verrouille tout de suite : la clé du coffre est effacée de la mémoire, après la
    /// publication en arrière-plan des modifications qui attendent encore.
    pub fn lock(&self) {
        let open = self.open().take();
        if let Some(open) = open {
            self.close(open);
        }
    }

    fn close(&self, open: OpenVault) {
        let Some(mut session) = open.lock.close() else {
            return;
        };
        if !session.has_pending_changes() {
            return;
        }
        let vaults = self.vaults.clone();
        let syncing = Arc::clone(&self.syncing);
        thread::spawn(move || {
            let _one_at_a_time = lock(&syncing);
            // Au mieux : le coffre est fermé, personne n'attend le résultat ; ce qui n'est pas
            // publié le sera à la prochaine ouverture.
            let _ = vaults.sync(&open.name, &mut session);
        });
    }

    /// Attend la fin de la synchro ou de l'échange en cours, et empêche le suivant de commencer.
    pub(crate) fn exclusive(&self) -> MutexGuard<'_, ()> {
        lock(&self.syncing)
    }

    /// Retient l'issue d'un contact avec le serveur : une erreur de port vient de lui. Une série
    /// d'échecs garde l'heure du premier.
    pub(crate) fn observe<T>(&self, result: Result<T, AppError>) -> Result<T, AppError> {
        let mut trouble = lock(&self.trouble);
        match &result {
            Ok(_) => *trouble = None,
            Err(AppError::Port(error)) => {
                let since = trouble
                    .as_ref()
                    .map_or_else(self.clock, |known| known.since);
                *trouble = Some(ServerTrouble::new(since, error.to_string()));
            }
            Err(_) => {}
        }
        result
    }

    pub(crate) fn trouble(&self) -> Option<ServerTrouble> {
        lock(&self.trouble).clone()
    }

    /// Synchronise le coffre ouvert en arrière-plan ; l'interface est prévenue du résultat.
    fn sync_later(&self) {
        let vaults = self.vaults.clone();
        let open = Arc::clone(&self.open);
        let syncing = Arc::clone(&self.syncing);
        let notifier = Arc::clone(&self.notifier);
        thread::spawn(move || {
            if let Some(report) = sync_open(&vaults, &open, &syncing, Trigger::Automatic) {
                notifier.vault_synced(&report);
            }
        });
    }

    /// Synchronise le coffre ouvert et attend le résultat (bouton « Synchroniser ») ; supprimé
    /// du serveur, il y est republié.
    pub fn sync_now(&self) -> Result<SyncReport, UiError> {
        sync_open(&self.vaults, &self.open, &self.syncing, Trigger::Requested)
            .ok_or_else(UiError::vault_locked)
    }

    /// L'application passe en arrière-plan : le coffre ouvert se verrouillera après
    /// `background_lock_seconds`, ou tout de suite si la configuration est illisible.
    pub fn hide(&self) {
        if !self.background_lock {
            return;
        }
        let delay = self
            .vaults
            .config()
            .map_or(Duration::ZERO, |config| config.background_lock());
        let since = (self.clock)();
        let delay = self.hidden().get_or_insert(Hidden { since, delay }).delay;
        if let Some(open) = self.open().as_ref() {
            open.lock.expire_within(delay);
        }
    }

    /// L'application revient au premier plan. Son minuteur a pu être suspendu avec elle : si
    /// l'horloge du système dit le délai dépassé, ou si elle a reculé, le coffre est verrouillé.
    /// Renvoie le coffre encore ouvert, comme [`AppState::current`].
    pub fn show(&self) -> Option<VaultInfo> {
        let hidden = self.hidden().take();
        if let Some(hidden) = hidden {
            let elapsed = (self.clock)().as_millis() - hidden.since.as_millis();
            match u64::try_from(elapsed) {
                Ok(elapsed) if Duration::from_millis(elapsed) < hidden.delay => {}
                _ => self.lock(),
            }
        }
        self.current()
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

    /// Efface tout de suite le mot de passe copié, s'il est encore dans le presse-papiers.
    pub fn clear_clipboard(&self) {
        self.clipboard
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clear_now();
    }

    /// Ajoute l'entrée et l'enregistre aussitôt.
    pub fn add_entry(&self, input: EntryInput) -> Result<EntryDetail, UiError> {
        let content = input.into_content(None)?;
        let now = (self.clock)();
        self.modify(|session| {
            let id = session.edit(|vault| vault.add(content, now))?;
            Ok(EntryDetail::from(find(session.vault(), id)?))
        })?
    }

    /// Remplace le contenu de l'entrée ; la version précédente rejoint l'historique.
    pub fn update_entry(&self, id: &str, input: EntryInput) -> Result<EntryDetail, UiError> {
        let id = parse_id(id)?;
        let now = (self.clock)();
        self.modify(|session| {
            let current = find(session.vault(), id)?.content().password.clone();
            let content = input.into_content(current)?;
            session.edit(|vault| vault.update(id, content, now))??;
            Ok(EntryDetail::from(find(session.vault(), id)?))
        })?
    }

    pub fn delete_entry(&self, id: &str) -> Result<(), UiError> {
        let id = parse_id(id)?;
        let now = (self.clock)();
        self.modify(|session| Ok(session.edit(|vault| vault.delete(id, now))??))?
    }

    /// Versions précédentes de l'entrée, sans mot de passe.
    pub fn history(&self, id: &str) -> Result<Vec<HistoryItem>, UiError> {
        let id = parse_id(id)?;
        self.with_vault(|vault| find(vault, id).map(history))?
    }

    /// Change le mot de passe maître du coffre ouvert ; l'actuel est redemandé.
    pub fn change_password(&self, current: &Secret, new: &Secret) -> Result<(), UiError> {
        let name = self.open_name()?;
        check_master_password(new)?;
        self.vaults.open(&name, Credentials::Password(current))?;
        let now = (self.clock)();
        self.modify(|session| session.change_password(new, self.kdf, now))??;
        Ok(())
    }

    /// Mot de passe oublié : la clé de récupération ouvre le coffre et fixe un nouveau mot de
    /// passe ; le coffre reste ouvert.
    pub fn recover(
        &self,
        name: &str,
        recovery_key: &Secret,
        new: &Secret,
    ) -> Result<VaultInfo, UiError> {
        let key = RecoveryKey::parse(recovery_key.expose()).map_err(|error| match error {
            RecoveryKeyError::Malformed => UiError::code("invalidRecoveryKey"),
            // Bien formée : l'utilisateur a recopié une autre clé, ou fait une faute de frappe.
            RecoveryKeyError::InvalidLastCharacter => UiError::code("wrongRecoveryKey"),
        })?;
        check_master_password(new)?;
        let timeout = self.auto_lock()?;
        let mut session = self.vaults.open(name, Credentials::RecoveryKey(&key))?;
        session.change_password(new, self.kdf, (self.clock)())?;
        Ok(self.keep_open(name, session, timeout))
    }

    /// Supprime un coffre de cet appareil ; `confirmation` doit être son nom, retapé. Une
    /// synchro en cours est d'abord attendue : elle pourrait réécrire le coffre.
    pub fn delete_vault(&self, name: &str, confirmation: &str) -> Result<(), UiError> {
        confirm(name, confirmation)?;
        let _one_at_a_time = self.exclusive();
        self.close_without_publishing(name);
        Ok(self.vaults.delete(name)?)
    }

    /// Ferme le coffre `name` s'il est ouvert, sans rien publier : il va être supprimé.
    pub(crate) fn close_without_publishing(&self, name: &str) {
        let open = self.open().take_if(|open| open.name == name);
        if let Some(open) = open {
            drop(open.lock.close());
        }
    }

    /// Importe un fichier CSV dans le coffre ouvert, tout ou rien ; renvoie le nombre d'entrées.
    pub fn import_entries(&self, data: &[u8]) -> Result<usize, UiError> {
        let contents = import_csv(data).map_err(AppError::Csv)?;
        let now = (self.clock)();
        Ok(self.modify(|session| add_entries(session, contents, now))??)
    }

    /// Nom proposé pour une sauvegarde : `pswd-sauvegarde_<date>.pswd`.
    pub fn backup_file_name(&self) -> String {
        backup_file_name((self.clock)())
    }

    /// Les coffres choisis en un seul fichier `.pswd`, et leur nombre.
    pub fn backup_file(&self, names: &[String]) -> Result<(Vec<u8>, usize), UiError> {
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        Ok(self.vaults.backup_to_file(Some(&names), (self.clock)())?)
    }

    pub fn backup_file_contents(&self, data: &[u8]) -> Result<Vec<String>, UiError> {
        Ok(self.vaults.backup_file_contents(data)?)
    }

    /// Restaure un coffre d'un fichier `.pswd` ; renvoie le nom retenu.
    pub fn restore_from_file(&self, data: &[u8], name: &str) -> Result<String, UiError> {
        Ok(self.vaults.restore_from_file(data, name)?)
    }

    pub fn settings(&self) -> Result<Settings, UiError> {
        Ok(Settings::from(&self.vaults.config()?))
    }

    pub fn save_settings(&self, settings: Settings) -> Result<Settings, UiError> {
        settings.check()?;
        let config = self
            .vaults
            .update_config(|config| settings.apply_to(config))?;
        Ok(Settings::from(&config))
    }

    fn open_name(&self) -> Result<String, UiError> {
        self.open()
            .as_ref()
            .map(|open| open.name.clone())
            .ok_or_else(UiError::vault_locked)
    }

    /// Lit le coffre ouvert, et repousse son verrouillage.
    fn with_vault<T>(&self, read: impl FnOnce(&Vault) -> T) -> Result<T, UiError> {
        self.with_session(|session| read(session.vault()))
    }

    /// Utilise la session du coffre ouvert, et repousse son verrouillage.
    fn with_session<T>(
        &self,
        use_session: impl FnOnce(&mut VaultSession) -> T,
    ) -> Result<T, UiError> {
        let open = self.open();
        let open = open.as_ref().ok_or_else(UiError::vault_locked)?;
        open.lock
            .with(use_session)
            .ok_or_else(UiError::vault_locked)
    }

    /// Modifie le coffre ouvert, puis le synchronise en arrière-plan.
    fn modify<T>(&self, change: impl FnOnce(&mut VaultSession) -> T) -> Result<T, UiError> {
        let result = {
            let mut open = self.open();
            let open = open.as_mut().ok_or_else(UiError::vault_locked)?;
            let result = open.lock.with(change).ok_or_else(UiError::vault_locked)?;
            open.edits += 1;
            result
        };
        self.sync_later();
        Ok(result)
    }

    fn open(&self) -> MutexGuard<'_, Option<OpenVault>> {
        lock(&self.open)
    }

    fn hidden(&self) -> MutexGuard<'_, Option<Hidden>> {
        lock(&self.hidden)
    }
}

/// Synchro automatique, ou demandée par l'utilisateur : seule la seconde republie un coffre
/// supprimé du serveur.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Trigger {
    Automatic,
    Requested,
}

/// Synchronise une copie de la session ouverte : l'interface continue de lire et de modifier le
/// coffre pendant l'échange avec le serveur. `None` si aucun coffre n'est ouvert. Un coffre dont
/// le compte n'est plus membre vient d'être supprimé : il est fermé.
fn sync_open(
    vaults: &Vaults,
    open: &Mutex<Option<OpenVault>>,
    syncing: &Mutex<()>,
    trigger: Trigger,
) -> Option<SyncReport> {
    let _one_at_a_time = lock(syncing);
    let (name, opening, edits, mut session) = {
        let open = lock(open);
        let open = open.as_ref()?;
        let session = open.lock.with(|session| session.clone())?;
        (open.name.clone(), open.opening, open.edits, session)
    };
    let outcome = match trigger {
        Trigger::Automatic => vaults.sync(&name, &mut session),
        Trigger::Requested => vaults.republish(&name, &mut session),
    };
    let changed = if outcome == Ok(SyncOutcome::Left) {
        let left = lock(open).take_if(|open| open.opening == opening);
        drop(left.and_then(|open| open.lock.close()));
        false
    } else {
        give_back(open, opening, edits, session)
    };
    Some(SyncReport::new(name, outcome, changed))
}

/// Rend la copie synchronisée au coffre ouvert, s'il n'a été ni fermé ni modifié entre-temps ;
/// sinon, la modification a prévu une autre synchro. Renvoie `true` si le contenu a changé.
fn give_back(
    open: &Mutex<Option<OpenVault>>,
    opening: u64,
    edits: u64,
    synced: VaultSession,
) -> bool {
    match lock(open).as_ref() {
        Some(open) if open.opening == opening && open.edits == edits => open
            .lock
            .with(|session| {
                let changed = session.vault() != synced.vault();
                *session = synced;
                changed
            })
            .unwrap_or(false),
        _ => false,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
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

/// Une suppression se confirme en retapant le nom de ce qui est supprimé.
pub(crate) fn confirm(name: &str, confirmation: &str) -> Result<(), UiError> {
    if confirmation.trim() == name {
        Ok(())
    } else {
        Err(UiError::code("confirmationMismatch"))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

    use pswd_core::{EntryContent, EntryKind, Tags};
    use pswd_infra::Locations;
    use pswd_server::testing::TestServer;
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::sync::SyncState;
    use crate::testing::FakeClipboard;

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    fn vaults_in(dir: &TempDir) -> Vaults {
        Vaults::new(Locations::new(
            &dir.path().join("config"),
            &dir.path().join("data"),
        ))
    }

    thread_local! {
        static NOW: Cell<i64> = const { Cell::new(1_000) };
    }

    /// Horloge propre à chaque test, avancée par [`advance`].
    fn test_clock() -> Timestamp {
        Timestamp::from_millis(NOW.with(Cell::get))
    }

    fn advance(millis: i64) {
        NOW.with(|now| now.set(now.get() + millis));
    }

    /// Interface factice : compte les verrouillages par inactivité et transmet les synchros.
    struct FakeNotifier {
        locks: Arc<AtomicUsize>,
        reports: Mutex<mpsc::Sender<SyncReport>>,
    }

    impl Notifier for FakeNotifier {
        fn vault_locked(&self) {
            self.locks.fetch_add(1, Ordering::SeqCst);
        }

        fn vault_synced(&self, report: &SyncReport) {
            let _ = self.reports.lock().unwrap().send(report.clone());
        }
    }

    fn notifier() -> (FakeNotifier, Arc<AtomicUsize>, mpsc::Receiver<SyncReport>) {
        let locks = Arc::new(AtomicUsize::new(0));
        let (sender, receiver) = mpsc::channel();
        let notifier = FakeNotifier {
            locks: Arc::clone(&locks),
            reports: Mutex::new(sender),
        };
        (notifier, locks, receiver)
    }

    /// État dans un dossier temporaire, le nombre de verrouillages par inactivité signalés et
    /// les synchros automatiques.
    fn build(
        dir: &TempDir,
        auto_lock: Option<Duration>,
        clipboard: FakeClipboard,
    ) -> (AppState, Arc<AtomicUsize>, mpsc::Receiver<SyncReport>) {
        let kdf = KdfParams::new(8 * 1024, 1, 1).unwrap();
        let (notifier, locks, reports) = notifier();
        let state = AppState::with_settings(
            vaults_in(dir),
            Box::new(clipboard),
            kdf,
            test_clock,
            auto_lock,
            true,
            notifier,
        );
        (state, locks, reports)
    }

    fn state_with(
        dir: &TempDir,
        auto_lock: Option<Duration>,
        clipboard: FakeClipboard,
    ) -> (AppState, Arc<AtomicUsize>) {
        let (state, locks, _) = build(dir, auto_lock, clipboard);
        (state, locks)
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
        let (notifier, locks, _) = notifier();
        let clipboard = Box::new(FakeClipboard::default());
        let state = AppState::new(vaults_in(&dir), clipboard, notifier);
        state.notifier.vault_locked();
        assert_eq!(locks.load(Ordering::SeqCst), 1);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        state.lock();
        assert_eq!(
            state.unlock("perso", &secret("Maître-2026!")),
            Ok(info("perso", 0))
        );
    }

    #[test]
    fn created_vault_stays_open_and_gives_its_recovery_key() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (vault, recovery_key) = state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        assert_eq!(vault, info("perso", 0));
        assert_eq!(recovery_key.expose().split('-').count(), 13);
        assert_eq!(state.current(), Some(info("perso", 0)));
    }

    #[test]
    fn creation_errors_are_reported() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        assert_eq!(
            state.create("perso", &secret(""), false).err(),
            Some(UiError::code("passwordTooShort"))
        );
        assert_eq!(
            state
                .create("perso", &secret("mot de passe maître"), false)
                .err(),
            Some(UiError::code("passwordTooSimple"))
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
                &secret("Maître-2026!"),
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
            state.unlock("perso", &secret("Maître-2026!")),
            Ok(info("perso", 1))
        );
        assert_eq!(state.current(), Some(info("perso", 1)));
    }

    #[test]
    fn only_one_vault_is_open_at_a_time() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        state
            .create("travail", &secret("Autre-mot-2026!"), false)
            .unwrap();
        assert_eq!(state.current(), Some(info("travail", 0)));
        state.unlock("perso", &secret("Maître-2026!")).unwrap();
        assert_eq!(state.current(), Some(info("perso", 0)));
    }

    #[test]
    fn locking_closes_the_vault_without_notification() {
        let dir = TempDir::new().unwrap();
        let (state, count) = state_in(&dir, Some(Duration::from_millis(50)));
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
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
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        thread::sleep(Duration::from_millis(300));
        assert_eq!(state.current(), None);
        assert_eq!(count.load(Ordering::SeqCst), 1);
        state.lock();
        assert_eq!(state.current(), None);
    }

    fn lock_in_background_after(state: &AppState, seconds: u32) {
        state
            .vaults
            .update_config(|config| config.background_lock_seconds = seconds)
            .unwrap();
    }

    #[test]
    fn vault_locks_at_once_in_background_when_the_delay_is_zero() {
        let dir = TempDir::new().unwrap();
        let (state, count) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        lock_in_background_after(&state, 0);
        state.hide();
        thread::sleep(Duration::from_millis(300));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(state.show(), None);
    }

    #[test]
    fn vault_stays_open_after_a_short_absence() {
        let dir = TempDir::new().unwrap();
        let (state, count) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        state.hide();
        advance(59_999);
        assert_eq!(state.show(), Some(info("perso", 0)));
        thread::sleep(Duration::from_millis(100));
        assert_eq!(state.current(), Some(info("perso", 0)));
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn long_absence_locks_the_vault_on_return() {
        let dir = TempDir::new().unwrap();
        let (state, count) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        state.hide();
        advance(60_000);
        assert_eq!(state.show(), None);
        assert_eq!(state.current(), None);
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn clock_going_back_locks_the_vault_on_return() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        state.hide();
        advance(-1);
        assert_eq!(state.show(), None);
    }

    #[test]
    fn hiding_twice_keeps_the_first_moment() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        state.hide();
        advance(30_000);
        state.hide();
        advance(30_000);
        assert_eq!(state.show(), None);
    }

    #[test]
    fn hiding_without_an_open_vault_changes_nothing() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state.hide();
        assert_eq!(state.show(), None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        advance(3_600_000);
        assert_eq!(state.show(), Some(info("perso", 0)));
    }

    #[test]
    fn showing_without_hiding_changes_nothing() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        assert_eq!(state.show(), None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        advance(3_600_000);
        assert_eq!(state.show(), Some(info("perso", 0)));
    }

    #[test]
    fn unreadable_configuration_locks_at_once_in_background() {
        let dir = TempDir::new().unwrap();
        let (state, count) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        std::fs::write(dir.path().join("config").join("config.toml"), "couleur = 1").unwrap();
        state.hide();
        thread::sleep(Duration::from_millis(300));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(state.show(), None);
    }

    #[cfg(desktop)]
    #[test]
    fn desktop_ignores_the_background() {
        let dir = TempDir::new().unwrap();
        let clipboard = Box::new(FakeClipboard::default());
        let (notifier, count, _) = notifier();
        let state = AppState::new(vaults_in(&dir), clipboard, notifier);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        lock_in_background_after(&state, 0);
        state.hide();
        thread::sleep(Duration::from_millis(300));
        assert_eq!(state.show(), Some(info("perso", 0)));
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn default_delay_comes_from_the_configuration() {
        let dir = TempDir::new().unwrap();
        let (state, count) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        thread::sleep(Duration::from_millis(100));
        assert_eq!(state.current(), Some(info("perso", 0)));
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn unreadable_configuration_keeps_the_vault_closed() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        state.lock();
        let config = dir.path().join("config").join("config.toml");
        let text = std::fs::read_to_string(&config).unwrap();
        std::fs::write(&config, format!("{text}\nauto_lock_minutes = \"dix\"\n")).unwrap();
        assert!(state.unlock("perso", &secret("Maître-2026!")).is_err());
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
                &secret("Maître-2026!"),
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
        state.unlock("perso", &secret("Maître-2026!")).unwrap();
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
        let (github, _, _) = open_with_entries(&state);
        thread::sleep(Duration::from_millis(400));
        let locked = Some(UiError::vault_locked());
        assert_eq!(state.entries("", None).err(), locked);
        assert_eq!(state.delete_entry(&github).err(), locked);
        assert_eq!(state.sync_now().err(), locked);
    }

    fn entry_input(value: serde_json::Value) -> EntryInput {
        serde_json::from_value(value).unwrap()
    }

    fn wifi_input(password: &serde_json::Value) -> EntryInput {
        entry_input(json!({
            "kind": "login",
            "title": "Box",
            "username": "admin",
            "password": password,
            "urls": [],
            "notes": "",
            "tags": ["maison"],
        }))
    }

    #[test]
    fn editing_needs_an_open_vault() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let id = Uuid::nil().to_string();
        let locked = Some(UiError::vault_locked());
        let keep = || wifi_input(&json!({ "mode": "keep" }));
        assert_eq!(state.add_entry(keep()).err(), locked);
        assert_eq!(state.update_entry(&id, keep()).err(), locked);
        assert_eq!(state.delete_entry(&id).err(), locked);
        assert_eq!(state.history(&id).err(), locked);
    }

    #[test]
    fn added_entry_is_saved_and_listed() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        open_with_entries(&state);
        let added = state
            .add_entry(wifi_input(&json!({ "mode": "set", "value": "box-1234" })))
            .unwrap();
        let id = serde_json::to_value(&added).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(state.reveal_password(&id).unwrap().expose(), "box-1234");
        assert_eq!(
            titles(&state.entries("", None).unwrap()),
            ["banque", "Box", "GitHub", "Wi-Fi"]
        );
        state.lock();
        state.unlock("perso", &secret("Maître-2026!")).unwrap();
        assert_eq!(titles(&state.entries("box", None).unwrap()), ["Box"]);
    }

    #[test]
    fn invalid_input_adds_nothing() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        open_with_entries(&state);
        let blank = entry_input(json!({
            "kind": "note",
            "title": "",
            "username": "",
            "password": { "mode": "keep" },
            "urls": [],
            "notes": "",
            "tags": [],
        }));
        assert_eq!(
            state.add_entry(blank).err(),
            Some(UiError::code("emptyTitle"))
        );
        assert_eq!(state.entries("", None).unwrap().len(), 3);
    }

    #[test]
    fn updated_entry_keeps_its_password_and_its_history() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (github, _, _) = open_with_entries(&state);
        let updated = state
            .update_entry(&github, wifi_input(&json!({ "mode": "keep" })))
            .unwrap();
        let updated = serde_json::to_value(updated).unwrap();
        assert_eq!(updated["title"], "Box");
        assert_eq!(updated["versions"], 1);
        assert_eq!(state.reveal_password(&github).unwrap().expose(), "hunter2");
        let history = serde_json::to_value(state.history(&github).unwrap()).unwrap();
        assert_eq!(history[0]["title"], "GitHub");
        assert_eq!(history[0]["passwordChanged"], false);

        state
            .update_entry(
                &github,
                wifi_input(&json!({ "mode": "generate", "length": 20, "symbols": true })),
            )
            .unwrap();
        assert_eq!(
            state
                .reveal_password(&github)
                .unwrap()
                .expose()
                .chars()
                .count(),
            20
        );
        let history = serde_json::to_value(state.history(&github).unwrap()).unwrap();
        assert_eq!(history[0]["passwordChanged"], true);
    }

    #[test]
    fn missing_entries_cannot_be_edited() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (_, _, deleted) = open_with_entries(&state);
        let not_found = Some(UiError::new("entryNotFound", deleted.clone()));
        let keep = || wifi_input(&json!({ "mode": "keep" }));
        assert_eq!(state.update_entry(&deleted, keep()).err(), not_found);
        assert_eq!(state.delete_entry(&deleted).err(), not_found);
        assert_eq!(state.history(&deleted).err(), not_found);
        let bad = Some(UiError::new("entryNotFound", "x"));
        assert_eq!(state.update_entry("x", keep()).err(), bad);
        assert_eq!(state.delete_entry("x").err(), bad);
        assert_eq!(state.history("x").err(), bad);
    }

    #[test]
    fn invalid_update_changes_nothing() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (github, _, _) = open_with_entries(&state);
        let too_short = wifi_input(&json!({ "mode": "generate", "length": 2, "symbols": true }));
        assert_eq!(
            state.update_entry(&github, too_short).err(),
            Some(UiError::code("invalidLength"))
        );
        assert_eq!(
            serde_json::to_value(state.entry(&github).unwrap()).unwrap()["title"],
            "GitHub"
        );
    }

    #[test]
    fn deleted_entry_disappears() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (github, _, _) = open_with_entries(&state);
        assert_eq!(state.delete_entry(&github), Ok(()));
        assert_eq!(
            titles(&state.entries("", None).unwrap()),
            ["banque", "Wi-Fi"]
        );
        assert!(state.entry(&github).is_err());
    }

    #[test]
    fn failed_save_is_reported() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        open_with_entries(&state);
        let file = dir
            .path()
            .join("data")
            .join("vaults")
            .join("perso")
            .join("vault.enc.tmp");
        std::fs::create_dir(&file).unwrap();
        let error = state
            .add_entry(wifi_input(&json!({ "mode": "clear" })))
            .unwrap_err();
        assert_eq!(serde_json::to_value(error).unwrap()["code"], "saveFailed");
        assert_eq!(state.entries("", None).unwrap().len(), 3);
    }

    #[test]
    fn managing_needs_an_open_vault() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let locked = Some(UiError::vault_locked());
        assert_eq!(
            state.change_password(&secret("a"), &secret("b")).err(),
            locked
        );
        let csv = b"title,kind,username,password,urls,notes,tags\r\n";
        assert_eq!(state.import_entries(csv).err(), locked);
    }

    #[test]
    fn master_password_is_changed_after_checking_the_current_one() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        assert_eq!(
            state
                .change_password(&secret("faux"), &secret("Nouveau-2026!"))
                .err(),
            Some(UiError::code("wrongPassword"))
        );
        assert_eq!(
            state
                .change_password(&secret("Maître-2026!"), &secret(""))
                .err(),
            Some(UiError::code("passwordTooShort"))
        );
        assert_eq!(
            state.change_password(&secret("Maître-2026!"), &secret("Nouveau-2026!")),
            Ok(())
        );
        state.lock();
        assert!(state.unlock("perso", &secret("Maître-2026!")).is_err());
        assert_eq!(
            state.unlock("perso", &secret("Nouveau-2026!")),
            Ok(info("perso", 0))
        );
    }

    #[test]
    fn recovery_key_sets_a_new_password_and_opens_the_vault() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (_, key) = state
            .create("perso", &secret("Oublié-mot-2026!"), false)
            .unwrap();
        state.lock();
        let other = secret(&format!(" {} ", key.expose().to_lowercase()));
        assert_eq!(
            state.recover("perso", &other, &secret("Nouveau-2026!")),
            Ok(info("perso", 0))
        );
        assert_eq!(state.current(), Some(info("perso", 0)));
        state.lock();
        assert_eq!(
            state.unlock("perso", &secret("Nouveau-2026!")),
            Ok(info("perso", 0))
        );
    }

    #[test]
    fn recovery_errors_are_reported() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let (_, key) = state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        state.lock();
        for typed in ["pas une clé".to_owned(), key.expose().replace('-', " ")] {
            assert_eq!(
                state.recover("perso", &secret(&typed), &secret("x")).err(),
                Some(UiError::code("invalidRecoveryKey")),
                "{typed}"
            );
        }
        let impossible = format!("{}H", &key.expose()[..key.expose().len() - 1]);
        assert_eq!(
            state
                .recover("perso", &secret(&impossible), &secret("Nouveau-2026!"))
                .err(),
            Some(UiError::code("wrongRecoveryKey"))
        );
        assert_eq!(
            state.recover("perso", &key, &secret("")).err(),
            Some(UiError::code("passwordTooShort"))
        );
        let other_key = RecoveryKey::generate().unwrap().encode();
        assert_eq!(
            state
                .recover("perso", &other_key, &secret("Nouveau-2026!"))
                .err(),
            Some(UiError::code("wrongRecoveryKey"))
        );
        assert_eq!(state.current(), None);
    }

    #[test]
    fn a_vault_is_deleted_once_its_name_is_retyped() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state
            .create("travail", &secret("Travail-mot-2026!"), false)
            .unwrap();
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        assert_eq!(
            state.delete_vault("perso", "Perso").err(),
            Some(UiError::code("confirmationMismatch"))
        );
        assert_eq!(state.current(), Some(info("perso", 0)));
        assert_eq!(state.delete_vault("travail", " travail "), Ok(()));
        assert_eq!(state.current(), Some(info("perso", 0)));
        assert_eq!(state.delete_vault("perso", "perso"), Ok(()));
        assert_eq!(state.current(), None);
        assert!(state.vaults.config().unwrap().vaults.is_empty());
        assert_eq!(
            state.delete_vault("perso", "perso").err(),
            Some(UiError::new("unknownVault", "perso"))
        );
    }

    #[test]
    fn csv_files_are_imported_all_or_nothing() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        open_with_entries(&state);
        let csv = b"title,kind,username,password,urls,notes,tags\r\n\
             Box,login,admin,1234,,,maison\r\n\
             Code,note,,,,ABCD,\r\n";
        assert_eq!(state.import_entries(csv), Ok(2));
        assert_eq!(state.entries("", None).unwrap().len(), 5);
        let csv = b"title,kind,username,password,urls,notes,tags\r\nA,carte,,,,,\r\n";
        assert_eq!(
            state.import_entries(csv).err(),
            Some(UiError::new("csvUnknownKind", "2"))
        );
        assert_eq!(state.entries("", None).unwrap().len(), 5);
    }

    #[test]
    fn vaults_are_backed_up_to_a_file_then_restored() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        state
            .create("travail", &secret("Maître-2026!"), false)
            .unwrap();
        state
            .create("famille", &secret("Maître-2026!"), false)
            .unwrap();
        assert_eq!(
            state.backup_file_name(),
            "pswd-sauvegarde_1970-01-01_00-00-01.pswd"
        );
        let chosen = ["perso".to_owned(), "travail".to_owned()];
        let (file, count) = state.backup_file(&chosen).unwrap();
        assert_eq!(count, 2);
        assert_eq!(state.backup_file_contents(&file), Ok(chosen.to_vec()));
        assert_eq!(
            state.restore_from_file(&file, "perso"),
            Ok("perso-sauvegarde-1970-01-01".to_owned())
        );
        assert_eq!(
            state.restore_from_file(&file, "autre").err(),
            Some(UiError::new("notInBackup", "autre"))
        );
        assert_eq!(state.vaults.config().unwrap().vaults.len(), 4);
    }

    #[test]
    fn unusable_backup_files_are_reported() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let code = |error: UiError| serde_json::to_value(error).unwrap()["code"].clone();
        assert_eq!(code(state.backup_file(&[]).unwrap_err()), "nothingToBackUp");
        assert_eq!(
            code(state.backup_file(&["perso".to_owned()]).unwrap_err()),
            "unknownVault"
        );
        let foreign = b"pas une sauvegarde";
        assert_eq!(
            code(state.backup_file_contents(foreign).unwrap_err()),
            "invalidBackup"
        );
        assert_eq!(
            code(state.restore_from_file(foreign, "perso").unwrap_err()),
            "invalidBackup"
        );
    }

    #[test]
    fn settings_are_read_checked_and_saved() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        let defaults = serde_json::to_value(state.settings().unwrap()).unwrap();
        assert_eq!(defaults["autoLockMinutes"], 10);
        let changed: Settings = serde_json::from_value(json!({
            "autoLockMinutes": 5,
            "clipboardClearSeconds": 30,
            "backgroundLockSeconds": 0,
            "theme": "dark",
        }))
        .unwrap();
        assert_eq!(state.save_settings(changed), Ok(changed));
        assert_eq!(state.settings(), Ok(changed));
        assert_eq!(state.vaults.config().unwrap().auto_lock_minutes, 5);
        let invalid: Settings = serde_json::from_value(json!({
            "autoLockMinutes": 5,
            "clipboardClearSeconds": 1,
            "backgroundLockSeconds": 0,
            "theme": "dark",
        }))
        .unwrap();
        assert_eq!(
            state.save_settings(invalid).err(),
            Some(UiError::code("invalidSettings"))
        );
        let config = dir.path().join("config").join("config.toml");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, "couleur = 1").unwrap();
        assert!(state.settings().is_err());
        assert!(state.save_settings(changed).is_err());
    }

    /// Délai d'attente d'une synchro faite en arrière-plan.
    const SYNC_WAIT: Duration = Duration::from_secs(30);

    fn next_report(reports: &mpsc::Receiver<SyncReport>) -> SyncReport {
        reports.recv_timeout(SYNC_WAIT).unwrap()
    }

    fn report(vault: &str, state: SyncState, changed: bool) -> SyncReport {
        SyncReport {
            vault: vault.to_owned(),
            state,
            detail: None,
            changed,
        }
    }

    /// Connecte l'état, comme administrateur, à un serveur neuf.
    fn connect_admin(state: &AppState) -> TestServer {
        let server = TestServer::start();
        state
            .connect(&server.address, &server.admin_key, "PC", "Arthur")
            .unwrap();
        server
    }

    /// Connecte `other` au compte de `connected`, puis télécharge ses coffres.
    fn connect_same_account(connected: &AppState, other: &AppState) {
        let address = connected.vaults.server().unwrap().unwrap().local_address;
        let key = connected.new_device_key().unwrap();
        other.connect(&address, &key, "Portable", "").unwrap();
        other.exchange().unwrap();
    }

    #[test]
    fn a_vault_left_from_another_device_is_closed_and_deleted() {
        let dir = TempDir::new().unwrap();
        let (pc, _, pc_reports) = build(&dir, None, FakeClipboard::default());
        connect_admin(&pc);
        pc.create("famille", &secret("Maître-2026!"), true).unwrap();
        next_report(&pc_reports);
        pc.lock();

        let other = TempDir::new().unwrap();
        let (phone, _, phone_reports) = build(&other, None, FakeClipboard::default());
        connect_same_account(&pc, &phone);
        phone.unlock("famille", &secret("Maître-2026!")).unwrap();
        next_report(&phone_reports);
        pc.vaults.leave("famille").unwrap();

        assert_eq!(
            phone.sync_now(),
            Ok(report("famille", SyncState::Left, false))
        );
        assert_eq!(phone.current(), None);
        assert!(phone.vaults.config().unwrap().vaults.is_empty());
    }

    #[test]
    fn a_vault_deleted_from_the_server_is_republished_on_request() {
        let dir = TempDir::new().unwrap();
        let (pc, _, pc_reports) = build(&dir, None, FakeClipboard::default());
        connect_admin(&pc);
        pc.create("perso", &secret("Maître-2026!"), false).unwrap();
        next_report(&pc_reports);
        pc.lock();

        let other = TempDir::new().unwrap();
        let (phone, _, phone_reports) = build(&other, None, FakeClipboard::default());
        connect_same_account(&pc, &phone);
        phone.unlock("perso", &secret("Maître-2026!")).unwrap();
        next_report(&phone_reports);
        pc.vaults.delete_everywhere("perso").unwrap();
        phone
            .add_entry(wifi_input(&json!({ "mode": "set", "value": "box-1234" })))
            .unwrap();
        assert_eq!(
            next_report(&phone_reports),
            report("perso", SyncState::Removed, false)
        );
        assert_eq!(
            phone.sync_now(),
            Ok(report("perso", SyncState::Synced, false))
        );
    }

    #[test]
    fn a_vault_of_a_disconnected_device_is_not_synced() {
        let dir = TempDir::new().unwrap();
        let (state, _, reports) = build(&dir, None, FakeClipboard::default());
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        assert_eq!(
            next_report(&reports),
            report("perso", SyncState::NotConnected, false)
        );
    }

    #[test]
    fn opened_and_modified_vaults_are_synced_in_the_background() {
        let dir = TempDir::new().unwrap();
        let (pc, _, pc_reports) = build(&dir, None, FakeClipboard::default());
        connect_admin(&pc);
        pc.create("perso", &secret("Maître-2026!"), false).unwrap();
        assert_eq!(
            next_report(&pc_reports),
            report("perso", SyncState::Synced, false)
        );

        let other = TempDir::new().unwrap();
        let (phone, _, phone_reports) = build(&other, None, FakeClipboard::default());
        connect_same_account(&pc, &phone);
        phone.unlock("perso", &secret("Maître-2026!")).unwrap();
        assert_eq!(
            next_report(&phone_reports),
            report("perso", SyncState::Synced, false)
        );

        pc.add_entry(wifi_input(&json!({ "mode": "set", "value": "box-1234" })))
            .unwrap();
        assert_eq!(
            next_report(&pc_reports),
            report("perso", SyncState::Synced, false)
        );
        assert_eq!(
            phone.sync_now(),
            Ok(report("perso", SyncState::Synced, true))
        );
        assert_eq!(titles(&phone.entries("", None).unwrap()), ["Box"]);
        assert_eq!(
            phone.sync_now(),
            Ok(report("perso", SyncState::Synced, false))
        );
    }

    /// Version du coffre `perso` sur le serveur de `state`.
    fn published_version(state: &AppState) -> Option<String> {
        let client = state.vaults.server().unwrap().unwrap().client().unwrap();
        client.list_vaults().unwrap()[0]
            .version
            .clone()
            .map(|version| version.0)
    }

    #[test]
    fn locking_publishes_the_changes_still_waiting() {
        let dir = TempDir::new().unwrap();
        let (state, _, reports) = build(&dir, None, FakeClipboard::default());
        connect_admin(&state);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        next_report(&reports);
        assert_eq!(published_version(&state).as_deref(), Some("\"1\""));

        let busy = state.exclusive();
        state
            .add_entry(wifi_input(&json!({ "mode": "clear" })))
            .unwrap();
        state.lock();
        drop(busy);
        let start = std::time::Instant::now();
        while published_version(&state).as_deref() != Some("\"2\"") {
            assert!(start.elapsed() < SYNC_WAIT, "rien n'a été publié");
            thread::sleep(Duration::from_millis(20));
        }
        // La synchro prévue par la modification trouve le coffre fermé : rien à signaler.
        assert!(reports.recv_timeout(Duration::from_millis(200)).is_err());
    }

    #[test]
    fn a_synced_copy_is_given_back_only_to_an_unchanged_vault() {
        let dir = TempDir::new().unwrap();
        let (state, _) = state_in(&dir, None);
        state
            .create("perso", &secret("Maître-2026!"), false)
            .unwrap();
        let _no_background_sync = state.exclusive();
        let (opening, mut copy) = {
            let open = state.open();
            let open = open.as_ref().unwrap();
            (
                open.opening,
                open.lock.with(|session| session.clone()).unwrap(),
            )
        };
        copy.edit(|vault| {
            vault.add(
                EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
                test_clock(),
            )
        })
        .unwrap();
        assert!(!give_back(&state.open, opening + 1, 0, copy.clone()));
        assert!(!give_back(&state.open, opening, 1, copy.clone()));
        assert_eq!(state.current(), Some(info("perso", 0)));
        assert!(give_back(&state.open, opening, 0, copy.clone()));
        assert_eq!(state.current(), Some(info("perso", 1)));
        assert!(!give_back(&state.open, opening, 0, copy.clone()));
        state.lock();
        assert!(!give_back(&state.open, opening, 0, copy));
    }
}
