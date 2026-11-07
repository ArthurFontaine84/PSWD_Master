use std::fmt;

use crate::crypto::KdfParams;
use crate::ports::{LocalStore, PortError, PushError, RemoteFile, RemoteStore};
use crate::{FileError, RecoveryKey, Secret, Timestamp, Vault, VaultFile};

/// Nombre de tentatives quand le distant bouge entre la récupération et la publication.
const MAX_SYNC_ATTEMPTS: usize = 3;

/// Ce qui déverrouille le coffre.
#[derive(Debug, Clone, Copy)]
pub enum Credentials<'a> {
    Password(&'a Secret),
    RecoveryKey(&'a RecoveryKey),
    // EXTENSION(keyfile): mot de passe + fichier-clé.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    Synced,
    /// Distant injoignable : le coffre local reste utilisable.
    Offline(PortError),
    /// Le distant a changé à chaque tentative ; les modifications restent en attente.
    Conflict,
    /// Fichier distant inutilisable (altéré, autre coffre, version inconnue) : il n'est jamais
    /// écrasé.
    Rejected(FileError),
    /// Le distant a retiré le coffre, ou en a retiré l'utilisateur ; le coffre local reste
    /// utilisable et les modifications restent en attente.
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    NoLocalVault,
    LocalVaultExists,
    RemoteEmpty,
    Local(PortError),
    Remote(PortError),
    File(FileError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLocalVault => f.write_str("aucun coffre local : utiliser init"),
            Self::LocalVaultExists => f.write_str("un coffre existe déjà à cet emplacement local"),
            Self::RemoteEmpty => f.write_str("le distant ne contient aucun coffre"),
            Self::Local(error) => write!(f, "copie locale : {error}"),
            Self::Remote(error) => write!(f, "distant : {error}"),
            Self::File(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for SessionError {}

impl From<FileError> for SessionError {
    fn from(error: FileError) -> Self {
        Self::File(error)
    }
}

/// Coffre ouvert : toute modification est enregistrée localement, la publication se fait par
/// [`Session::sync`]. Une copie peut se synchroniser pendant que l'original reste consultable.
#[derive(Clone)]
pub struct Session<L> {
    local: L,
    file: VaultFile,
    vault: Vault,
    /// Dernier contenu lu ou écrit dans la copie locale : s'il a changé, un autre programme l'a
    /// réécrit.
    saved: Vec<u8>,
    pending: bool,
}

impl<L: LocalStore> Session<L> {
    /// Crée un coffre vide et l'enregistre localement. Renvoie aussi sa clé de récupération, à
    /// montrer une seule fois. Le coffre n'est publié nulle part.
    ///
    /// # Errors
    ///
    /// [`SessionError::LocalVaultExists`] si un coffre existe déjà (rien n'est alors écrit), ou
    /// une erreur d'écriture locale.
    pub fn create(
        local: L,
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
    ) -> Result<(Self, RecoveryKey), SessionError> {
        ensure_no_local_vault(&local)?;
        let (file, recovery_key) = VaultFile::create(password, params, now)?;
        let mut session = Self::new(local, file, Vault::new(), Vec::new());
        session.save_local()?;
        session.pending = true;
        Ok((session, recovery_key))
    }

    /// Rattache cet emplacement local à un coffre existant : le fichier distant n'est copié
    /// qu'après vérification des identifiants.
    ///
    /// # Errors
    ///
    /// [`SessionError::LocalVaultExists`] si un coffre existe déjà localement,
    /// [`SessionError::RemoteEmpty`] ou [`SessionError::Remote`] si le distant n'a pas de coffre
    /// ou est injoignable, une erreur de déverrouillage ou d'écriture locale.
    pub fn join(
        mut local: L,
        remote: &mut impl RemoteStore,
        credentials: Credentials<'_>,
    ) -> Result<Self, SessionError> {
        ensure_no_local_vault(&local)?;
        let remote_file = remote
            .fetch()
            .map_err(SessionError::Remote)?
            .ok_or(SessionError::RemoteEmpty)?;
        let (file, vault) = unlock(&remote_file.file, credentials)?;
        local
            .save_atomic(&remote_file.file)
            .map_err(SessionError::Local)?;
        Ok(Self::new(local, file, vault, remote_file.file))
    }

    /// Déverrouille la copie locale.
    ///
    /// # Errors
    ///
    /// [`SessionError::NoLocalVault`] s'il n'y a pas de copie locale, sinon une erreur de
    /// lecture ou de déverrouillage.
    pub fn open(local: L, credentials: Credentials<'_>) -> Result<Self, SessionError> {
        let saved = local
            .load()
            .map_err(SessionError::Local)?
            .ok_or(SessionError::NoLocalVault)?;
        let (file, vault) = unlock(&saved, credentials)?;
        Ok(Self::new(local, file, vault, saved))
    }

    fn new(local: L, file: VaultFile, vault: Vault, saved: Vec<u8>) -> Self {
        Self {
            local,
            file,
            vault,
            saved,
            pending: false,
        }
    }

    #[must_use]
    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    /// Des modifications enregistrées localement n'ont pas encore été publiées.
    #[must_use]
    pub fn has_pending_changes(&self) -> bool {
        self.pending
    }

    /// Date du dernier changement de mot de passe maître ou de clé de récupération, ici ou
    /// reçu d'une autre copie.
    #[must_use]
    pub fn keys_updated_at(&self) -> Timestamp {
        self.file.keys_updated_at()
    }

    /// Copie locale telle que la session l'a lue ou écrite en dernier. Après une synchro réussie,
    /// elle a le même contenu que le distant.
    #[must_use]
    pub fn saved_file(&self) -> &[u8] {
        &self.saved
    }

    /// Applique `change`. Si le coffre a changé, il est fusionné avec la copie locale, puis
    /// enregistré.
    ///
    /// # Errors
    ///
    /// Si la copie locale est illisible, absente ou inutilisable, ou si l'écriture échoue : le
    /// coffre revient alors à son état précédent et rien n'est écrit.
    pub fn edit<T>(&mut self, change: impl FnOnce(&mut Vault) -> T) -> Result<T, SessionError> {
        let before = self.vault.clone();
        let result = change(&mut self.vault);
        self.commit_edit(before)?;
        Ok(result)
    }

    // Hors de `edit`, qui est générique : ce code n'est compilé qu'une fois.
    fn commit_edit(&mut self, before: Vault) -> Result<(), SessionError> {
        if self.vault == before {
            return Ok(());
        }
        if let Err(error) = self.merge_local().and_then(|()| self.save_local()) {
            self.vault = before;
            return Err(error);
        }
        self.pending = true;
        Ok(())
    }

    /// Fusionne d'abord la copie locale : un mot de passe changé ailleurs entre-temps est
    /// remplacé par celui-ci.
    ///
    /// # Errors
    ///
    /// Si la copie locale est illisible, absente ou inutilisable, ou si le générateur aléatoire
    /// ou l'écriture locale échoue.
    pub fn change_password(
        &mut self,
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
    ) -> Result<(), SessionError> {
        self.merge_local()?;
        self.file.change_password(password, params, now)?;
        self.save_local()?;
        self.pending = true;
        Ok(())
    }

    /// Récupère le distant, fusionne, puis publie si le distant n'est pas à jour.
    ///
    /// # Errors
    ///
    /// Seulement si l'écriture locale ou le chiffrement échoue ; un distant injoignable ou
    /// inutilisable est signalé par le [`SyncStatus`] renvoyé.
    pub fn sync(&mut self, remote: &mut impl RemoteStore) -> Result<SyncStatus, SessionError> {
        let status = self.try_sync(remote)?;
        if status == SyncStatus::Synced {
            self.pending = false;
        }
        Ok(status)
    }

    fn try_sync(&mut self, remote: &mut impl RemoteStore) -> Result<SyncStatus, SessionError> {
        for _ in 0..MAX_SYNC_ATTEMPTS {
            let expected = match remote.fetch() {
                Err(error) => return Ok(unreachable_remote(error)),
                Ok(None) => None,
                Ok(Some(RemoteFile { file, version })) => {
                    let merge = match self.file.merge_remote(&self.vault, &file) {
                        Ok(merge) => merge,
                        Err(error) => return Ok(SyncStatus::Rejected(error)),
                    };
                    if merge.local_outdated {
                        self.vault = merge.vault;
                        self.merge_local()?;
                        self.save_local()?;
                    }
                    if !merge.remote_outdated {
                        return Ok(SyncStatus::Synced);
                    }
                    Some(version)
                }
            };
            let file = self.file.encrypt(&self.vault)?;
            match remote.push(&file, expected.as_ref()) {
                Ok(()) => return Ok(SyncStatus::Synced),
                Err(PushError::Conflict) => {}
                Err(PushError::Failed(error)) => return Ok(unreachable_remote(error)),
            }
        }
        Ok(SyncStatus::Conflict)
    }

    /// Fusionne la copie locale si un autre programme (CLI, interface graphique) l'a réécrite :
    /// l'écriture suivante ne perd rien de ce qu'il a enregistré. Notre propre écriture n'est pas
    /// refusionnée : plus ancienne que le coffre en mémoire, elle pourrait, à date égale, annuler
    /// la dernière modification. Un fichier disparu ou inutilisable n'est jamais remplacé.
    fn merge_local(&mut self) -> Result<(), SessionError> {
        let file = self
            .local
            .load()
            .map_err(SessionError::Local)?
            .ok_or(SessionError::NoLocalVault)?;
        if file != self.saved {
            self.vault = self.file.merge_remote(&self.vault, &file)?.vault;
        }
        Ok(())
    }

    fn save_local(&mut self) -> Result<(), SessionError> {
        let file = self.file.encrypt(&self.vault)?;
        self.local.save_atomic(&file).map_err(SessionError::Local)?;
        self.saved = file;
        Ok(())
    }
}

fn unreachable_remote(error: PortError) -> SyncStatus {
    if error.is_removed() {
        SyncStatus::Removed
    } else {
        SyncStatus::Offline(error)
    }
}

fn unlock(file: &[u8], credentials: Credentials<'_>) -> Result<(VaultFile, Vault), FileError> {
    match credentials {
        Credentials::Password(password) => VaultFile::unlock_with_password(file, password),
        Credentials::RecoveryKey(key) => VaultFile::unlock_with_recovery_key(file, key),
    }
}

fn ensure_no_local_vault(local: &impl LocalStore) -> Result<(), SessionError> {
    match local.load().map_err(SessionError::Local)? {
        Some(_) => Err(SessionError::LocalVaultExists),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::ports::RemoteVersion;
    use crate::{EntryContent, EntryKind, VaultError};

    #[derive(Default, Clone)]
    struct FakeLocal {
        file: Option<Vec<u8>>,
        saves: usize,
        /// Lecture impossible.
        broken: bool,
        /// Écriture impossible.
        full: bool,
    }

    impl LocalStore for FakeLocal {
        fn load(&self) -> Result<Option<Vec<u8>>, PortError> {
            if self.broken {
                return Err(PortError::new("disque illisible"));
            }
            Ok(self.file.clone())
        }

        fn save_atomic(&mut self, file: &[u8]) -> Result<(), PortError> {
            if self.full {
                return Err(PortError::new("disque plein"));
            }
            self.file = Some(file.to_vec());
            self.saves += 1;
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeRemote {
        file: Option<Vec<u8>>,
        version: u64,
        pushes: usize,
        offline: bool,
        push_fails: bool,
        /// Le distant répond « coffre retiré », dès la lecture ou seulement à la publication.
        removed: Option<Removal>,
        /// Fichiers publiés par un autre appareil juste avant nos prochaines publications.
        concurrent: Vec<Vec<u8>>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Removal {
        OnFetch,
        OnPush,
    }

    impl FakeRemote {
        fn holding(file: &[u8]) -> Self {
            Self {
                file: Some(file.to_vec()),
                ..Self::default()
            }
        }

        fn current_version(&self) -> Option<RemoteVersion> {
            self.file
                .as_ref()
                .map(|_| RemoteVersion(self.version.to_string()))
        }
    }

    impl RemoteStore for FakeRemote {
        fn fetch(&mut self) -> Result<Option<RemoteFile>, PortError> {
            if self.offline {
                return Err(PortError::new("hors ligne"));
            }
            if self.removed == Some(Removal::OnFetch) {
                return Err(PortError::removed("coffre retiré"));
            }
            Ok(self.file.clone().map(|file| RemoteFile {
                file,
                version: RemoteVersion(self.version.to_string()),
            }))
        }

        fn push(&mut self, file: &[u8], expected: Option<&RemoteVersion>) -> Result<(), PushError> {
            if self.push_fails {
                return Err(PushError::Failed(PortError::new("accès refusé")));
            }
            if self.removed == Some(Removal::OnPush) {
                return Err(PushError::Failed(PortError::removed("coffre retiré")));
            }
            if let Some(other) = self.concurrent.pop() {
                self.file = Some(other);
                self.version += 1;
            }
            if expected != self.current_version().as_ref() {
                return Err(PushError::Conflict);
            }
            self.file = Some(file.to_vec());
            self.version += 1;
            self.pushes += 1;
            Ok(())
        }
    }

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    fn fast_params() -> KdfParams {
        KdfParams::new(8 * 1024, 1, 1).unwrap()
    }

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    fn note(title: &str) -> EntryContent {
        EntryContent::new(EntryKind::Note, title.to_owned())
    }

    fn titles(vault: &Vault) -> Vec<&str> {
        let mut titles: Vec<&str> = vault
            .entries()
            .map(|entry| entry.content().title.as_str())
            .collect();
        titles.sort_unstable();
        titles
    }

    /// Un coffre « départ » chiffré avec le mot de passe « maître », et sa clé de récupération.
    fn initial_file() -> (Vec<u8>, RecoveryKey) {
        let (file, recovery_key) =
            VaultFile::create(&secret("maître"), fast_params(), at(0)).unwrap();
        let mut vault = Vault::new();
        vault.add(note("départ"), at(1));
        (file.encrypt(&vault).unwrap(), recovery_key)
    }

    /// Le même coffre modifié sur un autre appareil.
    fn changed_elsewhere(file: &[u8], title: &str) -> Vec<u8> {
        let (other, mut vault) = VaultFile::unlock_with_password(file, &secret("maître")).unwrap();
        vault.add(note(title), at(2));
        other.encrypt(&vault).unwrap()
    }

    fn decrypt(file: &[u8], password: &str) -> Vault {
        VaultFile::unlock_with_password(file, &secret(password))
            .unwrap()
            .1
    }

    fn local_holding(file: &[u8]) -> FakeLocal {
        FakeLocal {
            file: Some(file.to_vec()),
            ..FakeLocal::default()
        }
    }

    fn open(local: FakeLocal) -> Session<FakeLocal> {
        Session::open(local, Credentials::Password(&secret("maître"))).unwrap()
    }

    /// Session ouverte sur le coffre « départ », et un distant qui en a la même version.
    fn synced() -> (Session<FakeLocal>, FakeRemote, Vec<u8>) {
        let (file, _) = initial_file();
        (open(local_holding(&file)), FakeRemote::holding(&file), file)
    }

    #[test]
    fn open_requires_a_readable_local_vault_and_the_right_password() {
        let (file, _) = initial_file();
        let open_with = |local: FakeLocal, password: &str| {
            Session::open(local, Credentials::Password(&secret(password))).err()
        };
        assert_eq!(
            open_with(FakeLocal::default(), "maître"),
            Some(SessionError::NoLocalVault)
        );
        let broken = FakeLocal {
            broken: true,
            ..FakeLocal::default()
        };
        assert_eq!(
            open_with(broken, "maître"),
            Some(SessionError::Local(PortError::new("disque illisible")))
        );
        assert_eq!(
            open_with(local_holding(&file), "autre"),
            Some(SessionError::File(FileError::WrongPassword))
        );
    }

    #[test]
    fn open_reads_the_local_copy_without_writing() {
        let (file, _) = initial_file();
        let session = open(local_holding(&file));
        assert_eq!(titles(session.vault()), ["départ"]);
        assert!(!session.has_pending_changes());
        assert_eq!(session.local.saves, 0);
        assert_eq!(session.saved_file(), file);
    }

    #[test]
    fn recovery_key_opens_the_vault() {
        let (file, recovery_key) = initial_file();
        let session = Session::open(
            local_holding(&file),
            Credentials::RecoveryKey(&recovery_key),
        )
        .unwrap();
        assert_eq!(titles(session.vault()), ["départ"]);
    }

    #[test]
    fn edit_saves_locally_and_marks_the_change_as_pending() {
        let (file, _) = initial_file();
        let mut session = open(local_holding(&file));
        let id = session
            .edit(|vault| vault.add(note("nouvelle"), at(3)))
            .unwrap();
        assert!(session.vault().get(id).is_some());
        assert!(session.has_pending_changes());
        let saved = session.local.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(saved, "maître")), ["départ", "nouvelle"]);
        assert_eq!(session.saved_file(), saved);
    }

    #[test]
    fn edit_without_change_writes_nothing() {
        let (file, _) = initial_file();
        let mut session = open(local_holding(&file));
        let unknown = Uuid::nil();
        let result = session.edit(|vault| vault.delete(unknown, at(3))).unwrap();
        assert_eq!(result, Err(VaultError::EntryNotFound(unknown)));
        assert_eq!(session.local.saves, 0);
        assert!(!session.has_pending_changes());
    }

    #[test]
    fn failed_local_save_restores_the_vault() {
        let (file, _) = initial_file();
        let mut session = open(local_holding(&file));
        session.local.full = true;
        let result = session.edit(|vault| vault.add(note("perdue"), at(3)));
        assert_eq!(
            result.err(),
            Some(SessionError::Local(PortError::new("disque plein")))
        );
        assert_eq!(titles(session.vault()), ["départ"]);
        assert!(!session.has_pending_changes());
    }

    #[test]
    fn a_copy_syncs_without_changing_the_original() {
        let (mut session, _, file) = synced();
        session
            .edit(|vault| vault.add(note("nouvelle"), at(3)))
            .unwrap();
        let mut remote = FakeRemote::holding(&changed_elsewhere(&file, "ailleurs"));
        let mut copy = session.clone();
        assert_eq!(copy.sync(&mut remote), Ok(SyncStatus::Synced));
        assert_eq!(titles(copy.vault()), ["ailleurs", "départ", "nouvelle"]);
        assert!(!copy.has_pending_changes());
        assert_eq!(titles(session.vault()), ["départ", "nouvelle"]);
        assert!(session.has_pending_changes());
    }

    #[test]
    fn edit_keeps_what_another_program_saved_since_opening() {
        let (file, _) = initial_file();
        let mut session = open(local_holding(&file));
        session.local.file = Some(changed_elsewhere(&file, "ailleurs"));
        session
            .edit(|vault| vault.add(note("nouvelle"), at(3)))
            .unwrap();
        assert_eq!(titles(session.vault()), ["ailleurs", "départ", "nouvelle"]);
        let saved = session.local.file.as_deref().unwrap();
        assert_eq!(
            titles(&decrypt(saved, "maître")),
            ["ailleurs", "départ", "nouvelle"]
        );
    }

    #[test]
    fn edit_does_not_merge_back_its_own_earlier_save() {
        let (file, _) = initial_file();
        let mut session = open(local_holding(&file));
        let id = session
            .edit(|vault| vault.add(note("éphémère"), at(3)))
            .unwrap();
        session
            .edit(|vault| vault.delete(id, at(3)))
            .unwrap()
            .unwrap();
        assert!(session.vault().get(id).is_none());
        let saved = session.local.file.as_deref().unwrap();
        assert!(decrypt(saved, "maître").get(id).is_none());
    }

    #[test]
    fn edit_keeps_a_password_changed_by_another_program() {
        let (file, _) = initial_file();
        let mut session = open(local_holding(&file));
        let mut other = open(local_holding(&file));
        other
            .change_password(&secret("nouveau"), fast_params(), at(3))
            .unwrap();
        session.local.file.clone_from(&other.local.file);
        assert_ne!(session.keys_updated_at(), at(3));
        session
            .edit(|vault| vault.add(note("nouvelle"), at(4)))
            .unwrap();
        assert_eq!(session.keys_updated_at(), at(3));
        let saved = session.local.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(saved, "nouveau")), ["départ", "nouvelle"]);
    }

    #[test]
    fn edit_never_replaces_a_local_file_gone_or_unusable() {
        let (file, _) = initial_file();
        let (foreign, _) = initial_file();
        let unreadable = FakeLocal {
            broken: true,
            ..FakeLocal::default()
        };
        let cases = [
            (None, SessionError::NoLocalVault),
            (
                Some(foreign.clone()),
                SessionError::File(FileError::Corrupted),
            ),
        ];
        for (on_disk, expected) in cases {
            let mut session = open(local_holding(&file));
            session.local.file.clone_from(&on_disk);
            let result = session.edit(|vault| vault.add(note("perdue"), at(3)));
            assert_eq!(result.err(), Some(expected));
            assert_eq!(titles(session.vault()), ["départ"]);
            assert_eq!(session.local.file, on_disk);
            assert_eq!(session.local.saves, 0);
            assert!(!session.has_pending_changes());
        }
        let mut session = open(local_holding(&file));
        session.local = unreadable;
        assert_eq!(
            session.edit(|vault| vault.add(note("perdue"), at(3))).err(),
            Some(SessionError::Local(PortError::new("disque illisible")))
        );
        assert_eq!(titles(session.vault()), ["départ"]);
    }

    #[test]
    fn changed_password_replaces_one_changed_elsewhere_and_keeps_its_entries() {
        let (file, _) = initial_file();
        let mut session = open(local_holding(&file));
        let mut other = open(local_holding(&file));
        other
            .edit(|vault| vault.add(note("ailleurs"), at(2)))
            .unwrap();
        other
            .change_password(&secret("autre"), fast_params(), at(5))
            .unwrap();
        session.local.file.clone_from(&other.local.file);
        session
            .change_password(&secret("nouveau"), fast_params(), at(3))
            .unwrap();
        let saved = session.local.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(saved, "nouveau")), ["ailleurs", "départ"]);
        assert!(VaultFile::unlock_with_password(saved, &secret("autre")).is_err());
    }

    #[test]
    fn changed_password_needs_a_usable_local_file() {
        let (file, _) = initial_file();
        let mut session = open(local_holding(&file));
        session.local.file = None;
        assert_eq!(
            session
                .change_password(&secret("nouveau"), fast_params(), at(3))
                .err(),
            Some(SessionError::NoLocalVault)
        );
        assert_eq!(session.local.saves, 0);
        assert!(!session.has_pending_changes());
    }

    #[test]
    fn sync_keeps_what_another_program_saved_locally() {
        let (mut session, mut remote, file) = synced();
        remote.file = Some(changed_elsewhere(&file, "distante"));
        session.local.file = Some(changed_elsewhere(&file, "ailleurs"));
        assert_eq!(session.sync(&mut remote).unwrap(), SyncStatus::Synced);
        assert_eq!(titles(session.vault()), ["ailleurs", "distante", "départ"]);
        let saved = session.local.file.as_deref().unwrap();
        assert_eq!(
            titles(&decrypt(saved, "maître")),
            ["ailleurs", "distante", "départ"]
        );
    }

    #[test]
    fn changed_password_is_saved_and_pending() {
        let (file, _) = initial_file();
        let mut session = open(local_holding(&file));
        session
            .change_password(&secret("nouveau"), fast_params(), at(3))
            .unwrap();
        assert!(session.has_pending_changes());
        let saved = session.local.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(saved, "nouveau")), ["départ"]);
    }

    fn create(local: FakeLocal) -> Result<(Session<FakeLocal>, RecoveryKey), SessionError> {
        Session::create(local, &secret("maître"), fast_params(), at(0))
    }

    #[test]
    fn create_saves_an_empty_vault_waiting_for_publication() {
        let (session, recovery_key) = create(FakeLocal::default()).unwrap();
        assert_eq!(session.vault().entries().count(), 0);
        assert!(session.has_pending_changes());
        let saved = session.local.file.as_deref().unwrap();
        assert_eq!(decrypt(saved, "maître").id(), session.vault().id());
        let (_, recovered) = VaultFile::unlock_with_recovery_key(saved, &recovery_key).unwrap();
        assert_eq!(recovered.id(), session.vault().id());
    }

    #[test]
    fn create_never_replaces_an_existing_vault() {
        let (file, _) = initial_file();
        assert_eq!(
            create(local_holding(&file)).err(),
            Some(SessionError::LocalVaultExists)
        );
    }

    #[test]
    fn create_reports_a_local_write_failure() {
        assert_eq!(
            Session::create(
                BrokenOnSave(FakeLocal::default()),
                &secret("maître"),
                fast_params(),
                at(0)
            )
            .err(),
            Some(SessionError::Local(PortError::new("disque plein")))
        );
    }

    /// Lecture possible (vide) mais écriture impossible.
    struct BrokenOnSave(FakeLocal);

    impl LocalStore for BrokenOnSave {
        fn load(&self) -> Result<Option<Vec<u8>>, PortError> {
            self.0.load()
        }

        fn save_atomic(&mut self, _: &[u8]) -> Result<(), PortError> {
            Err(PortError::new("disque plein"))
        }
    }

    #[test]
    fn sync_with_an_identical_remote_writes_nothing() {
        let (mut session, mut remote, _) = synced();
        assert_eq!(session.sync(&mut remote).unwrap(), SyncStatus::Synced);
        assert_eq!(session.local.saves, 0);
        assert_eq!(remote.pushes, 0);
    }

    #[test]
    fn sync_brings_remote_changes_locally() {
        let (mut session, mut remote, file) = synced();
        remote.file = Some(changed_elsewhere(&file, "distante"));
        assert_eq!(session.sync(&mut remote).unwrap(), SyncStatus::Synced);
        assert_eq!(titles(session.vault()), ["distante", "départ"]);
        let saved = session.local.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(saved, "maître")), ["distante", "départ"]);
        assert_eq!(remote.pushes, 0);
    }

    #[test]
    fn sync_publishes_to_an_empty_remote() {
        let (mut session, _, _) = synced();
        let mut remote = FakeRemote::default();
        assert_eq!(session.sync(&mut remote).unwrap(), SyncStatus::Synced);
        let published = remote.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(published, "maître")), ["départ"]);
    }

    #[test]
    fn sync_publishes_pending_changes() {
        let (mut session, mut remote, _) = synced();
        session
            .edit(|vault| vault.add(note("nouvelle"), at(3)))
            .unwrap();
        assert_eq!(session.sync(&mut remote).unwrap(), SyncStatus::Synced);
        assert!(!session.has_pending_changes());
        let published = remote.file.as_deref().unwrap();
        assert_eq!(
            titles(&decrypt(published, "maître")),
            ["départ", "nouvelle"]
        );
    }

    #[test]
    fn offline_changes_stay_pending_until_the_next_sync() {
        let (mut session, mut remote, _) = synced();
        remote.offline = true;
        session
            .edit(|vault| vault.add(note("hors ligne"), at(3)))
            .unwrap();
        assert_eq!(
            session.sync(&mut remote).unwrap(),
            SyncStatus::Offline(PortError::new("hors ligne"))
        );
        assert!(session.has_pending_changes());

        remote.offline = false;
        assert_eq!(session.sync(&mut remote).unwrap(), SyncStatus::Synced);
        assert!(!session.has_pending_changes());
        let published = remote.file.as_deref().unwrap();
        assert_eq!(
            titles(&decrypt(published, "maître")),
            ["départ", "hors ligne"]
        );
    }

    #[test]
    fn concurrent_publication_is_merged_then_retried() {
        let (mut session, mut remote, file) = synced();
        remote.concurrent = vec![changed_elsewhere(&file, "concurrente")];
        session
            .edit(|vault| vault.add(note("locale"), at(3)))
            .unwrap();
        assert_eq!(session.sync(&mut remote).unwrap(), SyncStatus::Synced);
        assert_eq!(titles(session.vault()), ["concurrente", "départ", "locale"]);
        let published = remote.file.as_deref().unwrap();
        assert_eq!(
            titles(&decrypt(published, "maître")),
            ["concurrente", "départ", "locale"]
        );
    }

    #[test]
    fn sync_gives_up_after_three_conflicts() {
        let (mut session, mut remote, file) = synced();
        remote.concurrent = ["a", "b", "c"]
            .iter()
            .map(|title| changed_elsewhere(&file, title))
            .collect();
        session
            .edit(|vault| vault.add(note("locale"), at(3)))
            .unwrap();
        assert_eq!(session.sync(&mut remote).unwrap(), SyncStatus::Conflict);
        assert!(session.has_pending_changes());
        assert_eq!(remote.pushes, 0);
    }

    #[test]
    fn refused_push_is_reported_as_offline() {
        let (mut session, mut remote, _) = synced();
        remote.push_fails = true;
        session
            .edit(|vault| vault.add(note("locale"), at(3)))
            .unwrap();
        assert_eq!(
            session.sync(&mut remote).unwrap(),
            SyncStatus::Offline(PortError::new("accès refusé"))
        );
        assert!(session.has_pending_changes());
    }

    #[test]
    fn removed_remote_keeps_the_changes_pending() {
        let (mut session, mut remote, _) = synced();
        session
            .edit(|vault| vault.add(note("locale"), at(3)))
            .unwrap();
        remote.removed = Some(Removal::OnPush);
        assert_eq!(session.sync(&mut remote), Ok(SyncStatus::Removed));
        remote.removed = Some(Removal::OnFetch);
        assert_eq!(session.sync(&mut remote), Ok(SyncStatus::Removed));
        assert!(session.has_pending_changes());
        assert_eq!(remote.pushes, 0);
        assert_eq!(titles(session.vault()), ["départ", "locale"]);
    }

    #[test]
    fn unusable_remote_is_never_overwritten() {
        let (mut session, _, _) = synced();
        let (foreign, _) = initial_file();
        let mut remote = FakeRemote::holding(&foreign);
        session
            .edit(|vault| vault.add(note("locale"), at(3)))
            .unwrap();
        assert_eq!(
            session.sync(&mut remote).unwrap(),
            SyncStatus::Rejected(FileError::Corrupted)
        );
        assert_eq!(remote.file, Some(foreign));
        assert!(session.has_pending_changes());
    }

    #[test]
    fn sync_reports_a_local_write_failure() {
        let (mut session, mut remote, file) = synced();
        remote.file = Some(changed_elsewhere(&file, "distante"));
        session.local.full = true;
        assert_eq!(
            session.sync(&mut remote).err(),
            Some(SessionError::Local(PortError::new("disque plein")))
        );
    }

    #[test]
    fn changed_password_is_published() {
        let (mut session, mut remote, _) = synced();
        session
            .change_password(&secret("nouveau"), fast_params(), at(3))
            .unwrap();
        assert_eq!(session.sync(&mut remote).unwrap(), SyncStatus::Synced);
        let published = remote.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(published, "nouveau")), ["départ"]);
    }

    fn join(
        local: FakeLocal,
        remote: &mut FakeRemote,
        password: &str,
    ) -> Result<Session<FakeLocal>, SessionError> {
        Session::join(local, remote, Credentials::Password(&secret(password)))
    }

    #[test]
    fn join_copies_the_remote_vault_locally() {
        let (file, _) = initial_file();
        let mut remote = FakeRemote::holding(&file);
        let session = join(FakeLocal::default(), &mut remote, "maître").unwrap();
        assert_eq!(titles(session.vault()), ["départ"]);
        assert!(!session.has_pending_changes());
        assert_eq!(session.local.file.as_deref(), Some(file.as_slice()));
        assert_eq!(remote.pushes, 0);
    }

    #[test]
    fn join_with_the_recovery_key() {
        let (file, recovery_key) = initial_file();
        let session = Session::join(
            FakeLocal::default(),
            &mut FakeRemote::holding(&file),
            Credentials::RecoveryKey(&recovery_key),
        )
        .unwrap();
        assert_eq!(titles(session.vault()), ["départ"]);
    }

    #[test]
    fn join_never_replaces_a_local_vault() {
        let (file, _) = initial_file();
        assert_eq!(
            join(
                local_holding(&file),
                &mut FakeRemote::holding(&file),
                "maître"
            )
            .err(),
            Some(SessionError::LocalVaultExists)
        );
    }

    #[test]
    fn join_requires_a_remote_vault() {
        assert_eq!(
            join(FakeLocal::default(), &mut FakeRemote::default(), "maître").err(),
            Some(SessionError::RemoteEmpty)
        );
        let mut offline = FakeRemote {
            offline: true,
            ..FakeRemote::default()
        };
        assert_eq!(
            join(FakeLocal::default(), &mut offline, "maître").err(),
            Some(SessionError::Remote(PortError::new("hors ligne")))
        );
    }

    #[test]
    fn join_with_a_wrong_password_writes_nothing() {
        let (file, _) = initial_file();
        let local = FakeLocal::default();
        assert_eq!(
            join(local, &mut FakeRemote::holding(&file), "autre").err(),
            Some(SessionError::File(FileError::WrongPassword))
        );
    }

    #[test]
    fn join_reports_a_local_write_failure() {
        let (file, _) = initial_file();
        let local = BrokenOnSave(FakeLocal::default());
        assert_eq!(
            Session::join(
                local,
                &mut FakeRemote::holding(&file),
                Credentials::Password(&secret("maître"))
            )
            .err(),
            Some(SessionError::Local(PortError::new("disque plein")))
        );
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                SessionError::NoLocalVault,
                "aucun coffre local : utiliser init",
            ),
            (
                SessionError::LocalVaultExists,
                "un coffre existe déjà à cet emplacement local",
            ),
            (
                SessionError::RemoteEmpty,
                "le distant ne contient aucun coffre",
            ),
            (
                SessionError::Remote(PortError::new("hors ligne")),
                "distant : hors ligne",
            ),
            (
                SessionError::Local(PortError::new("disque plein")),
                "copie locale : disque plein",
            ),
            (
                SessionError::File(FileError::WrongPassword),
                "mot de passe maître incorrect",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
