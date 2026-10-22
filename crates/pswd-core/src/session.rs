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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    NoLocalVault,
    LocalVaultExists,
    RemoteVaultExists,
    RemoteEmpty,
    Local(PortError),
    Remote(PortError),
    File(FileError),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoLocalVault => f.write_str("aucun coffre local : utiliser init ou join"),
            Self::LocalVaultExists => f.write_str("un coffre existe déjà à cet emplacement local"),
            Self::RemoteVaultExists => {
                f.write_str("le dépôt distant contient déjà un coffre : utiliser join")
            }
            Self::RemoteEmpty => {
                f.write_str("le dépôt distant ne contient aucun coffre : utiliser init")
            }
            Self::Local(error) => write!(f, "copie locale : {error}"),
            Self::Remote(error) => write!(f, "dépôt distant : {error}"),
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

/// Coffre ouvert : toute modification est enregistrée localement puis publiée.
pub struct Session<L, R> {
    local: L,
    remote: R,
    file: VaultFile,
    vault: Vault,
    pending: bool,
    status: SyncStatus,
}

impl<L: LocalStore, R: RemoteStore> Session<L, R> {
    /// Crée un coffre vide, l'enregistre localement puis le publie. Renvoie aussi sa clé de
    /// récupération, à montrer une seule fois.
    ///
    /// Le distant doit être joignable : c'est le seul moyen de vérifier qu'il est vide.
    ///
    /// # Errors
    ///
    /// [`SessionError::LocalVaultExists`] ou [`SessionError::RemoteVaultExists`] si un coffre
    /// existe déjà (rien n'est alors écrit), [`SessionError::Remote`] si le distant est
    /// injoignable, ou une erreur d'écriture locale.
    pub fn create(
        local: L,
        mut remote: R,
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
    ) -> Result<(Self, RecoveryKey), SessionError> {
        ensure_no_local_vault(&local)?;
        if remote.fetch().map_err(SessionError::Remote)?.is_some() {
            return Err(SessionError::RemoteVaultExists);
        }
        let (file, recovery_key) = VaultFile::create(password, params, now)?;
        let mut session = Self::new(local, remote, file, Vault::new());
        session.save_local()?;
        session.pending = true;
        session.sync()?;
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
        mut remote: R,
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
        Ok(Self::new(local, remote, file, vault))
    }

    /// Déverrouille la copie locale puis tente une synchronisation. Si elle échoue, le coffre
    /// reste utilisable et [`Session::status`] indique pourquoi.
    ///
    /// # Errors
    ///
    /// [`SessionError::NoLocalVault`] s'il n'y a pas de copie locale, sinon une erreur de
    /// lecture, de déverrouillage ou d'écriture locale.
    pub fn open(local: L, remote: R, credentials: Credentials<'_>) -> Result<Self, SessionError> {
        let file = local
            .load()
            .map_err(SessionError::Local)?
            .ok_or(SessionError::NoLocalVault)?;
        let (file, vault) = unlock(&file, credentials)?;
        let mut session = Self::new(local, remote, file, vault);
        session.sync()?;
        Ok(session)
    }

    fn new(local: L, remote: R, file: VaultFile, vault: Vault) -> Self {
        Self {
            local,
            remote,
            file,
            vault,
            pending: false,
            status: SyncStatus::Synced,
        }
    }

    #[must_use]
    pub fn vault(&self) -> &Vault {
        &self.vault
    }

    #[must_use]
    pub fn status(&self) -> &SyncStatus {
        &self.status
    }

    /// Des modifications enregistrées localement n'ont pas encore été publiées.
    #[must_use]
    pub fn has_pending_changes(&self) -> bool {
        self.pending
    }

    /// Applique `change`. Si le coffre a changé, il est enregistré localement puis publié.
    ///
    /// # Errors
    ///
    /// Si l'écriture locale échoue : le coffre revient alors à son état précédent.
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
        if let Err(error) = self.save_local() {
            self.vault = before;
            return Err(error);
        }
        self.pending = true;
        self.sync()?;
        Ok(())
    }

    /// # Errors
    ///
    /// Si le générateur aléatoire ou l'écriture locale échoue.
    pub fn change_password(
        &mut self,
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
    ) -> Result<(), SessionError> {
        self.file.change_password(password, params, now)?;
        self.save_local()?;
        self.pending = true;
        self.sync()?;
        Ok(())
    }

    /// Récupère le distant, fusionne, puis publie si le distant n'est pas à jour.
    ///
    /// # Errors
    ///
    /// Seulement si l'écriture locale ou le chiffrement échoue ; un distant injoignable ou
    /// inutilisable est signalé par le [`SyncStatus`] renvoyé.
    pub fn sync(&mut self) -> Result<&SyncStatus, SessionError> {
        self.status = self.try_sync()?;
        if self.status == SyncStatus::Synced {
            self.pending = false;
        }
        Ok(&self.status)
    }

    fn try_sync(&mut self) -> Result<SyncStatus, SessionError> {
        for _ in 0..MAX_SYNC_ATTEMPTS {
            let expected = match self.remote.fetch() {
                Err(error) => return Ok(SyncStatus::Offline(error)),
                Ok(None) => None,
                Ok(Some(RemoteFile { file, version })) => {
                    let merge = match self.file.merge_remote(&self.vault, &file) {
                        Ok(merge) => merge,
                        Err(error) => return Ok(SyncStatus::Rejected(error)),
                    };
                    if merge.local_outdated {
                        self.vault = merge.vault;
                        self.save_local()?;
                    }
                    if !merge.remote_outdated {
                        return Ok(SyncStatus::Synced);
                    }
                    Some(version)
                }
            };
            let file = self.file.encrypt(&self.vault)?;
            match self.remote.push(&file, expected.as_ref()) {
                Ok(()) => return Ok(SyncStatus::Synced),
                Err(PushError::Conflict) => {}
                Err(PushError::Failed(error)) => return Ok(SyncStatus::Offline(error)),
            }
        }
        Ok(SyncStatus::Conflict)
    }

    fn save_local(&mut self) -> Result<(), SessionError> {
        let file = self.file.encrypt(&self.vault)?;
        self.local.save_atomic(&file).map_err(SessionError::Local)
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

    #[derive(Default)]
    struct FakeLocal {
        file: Option<Vec<u8>>,
        saves: usize,
        broken: bool,
    }

    impl LocalStore for FakeLocal {
        fn load(&self) -> Result<Option<Vec<u8>>, PortError> {
            if self.broken {
                return Err(PortError::new("disque illisible"));
            }
            Ok(self.file.clone())
        }

        fn save_atomic(&mut self, file: &[u8]) -> Result<(), PortError> {
            if self.broken {
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
        /// Fichiers publiés par un autre appareil juste avant nos prochaines publications.
        concurrent: Vec<Vec<u8>>,
    }

    impl FakeRemote {
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
            Ok(self.file.clone().map(|file| RemoteFile {
                file,
                version: RemoteVersion(self.version.to_string()),
            }))
        }

        fn push(&mut self, file: &[u8], expected: Option<&RemoteVersion>) -> Result<(), PushError> {
            if self.push_fails {
                return Err(PushError::Failed(PortError::new("accès refusé")));
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

    fn stores(file: &[u8]) -> (FakeLocal, FakeRemote) {
        let local = FakeLocal {
            file: Some(file.to_vec()),
            ..FakeLocal::default()
        };
        let remote = FakeRemote {
            file: Some(file.to_vec()),
            ..FakeRemote::default()
        };
        (local, remote)
    }

    fn open(local: FakeLocal, remote: FakeRemote) -> Session<FakeLocal, FakeRemote> {
        Session::open(local, remote, Credentials::Password(&secret("maître"))).unwrap()
    }

    #[test]
    fn open_requires_a_readable_local_vault_and_the_right_password() {
        let (file, _) = initial_file();
        let open_with = |local: FakeLocal, password: &str| {
            Session::open(
                local,
                FakeRemote::default(),
                Credentials::Password(&secret(password)),
            )
            .err()
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
            open_with(stores(&file).0, "autre"),
            Some(SessionError::File(FileError::WrongPassword))
        );
    }

    #[test]
    fn recovery_key_opens_the_vault() {
        let (file, recovery_key) = initial_file();
        let (local, remote) = stores(&file);
        let session =
            Session::open(local, remote, Credentials::RecoveryKey(&recovery_key)).unwrap();
        assert_eq!(titles(session.vault()), ["départ"]);
    }

    #[test]
    fn offline_open_uses_the_local_copy() {
        let (file, _) = initial_file();
        let (local, mut remote) = stores(&file);
        remote.offline = true;
        let session = open(local, remote);
        assert_eq!(titles(session.vault()), ["départ"]);
        assert_eq!(
            session.status(),
            &SyncStatus::Offline(PortError::new("hors ligne"))
        );
        assert!(!session.has_pending_changes());
    }

    #[test]
    fn open_with_an_identical_remote_writes_nothing() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        let session = open(local, remote);
        assert_eq!(session.status(), &SyncStatus::Synced);
        assert_eq!(session.local.saves, 0);
        assert_eq!(session.remote.pushes, 0);
    }

    #[test]
    fn open_brings_remote_changes_locally() {
        let (file, _) = initial_file();
        let (local, mut remote) = stores(&file);
        remote.file = Some(changed_elsewhere(&file, "distante"));
        let session = open(local, remote);
        assert_eq!(titles(session.vault()), ["distante", "départ"]);
        let saved = session.local.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(saved, "maître")), ["distante", "départ"]);
        assert_eq!(session.remote.pushes, 0);
    }

    #[test]
    fn open_publishes_to_an_empty_remote() {
        let (file, _) = initial_file();
        let (local, _) = stores(&file);
        let session = open(local, FakeRemote::default());
        assert_eq!(session.status(), &SyncStatus::Synced);
        let published = session.remote.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(published, "maître")), ["départ"]);
    }

    #[test]
    fn edit_saves_locally_and_publishes() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        let mut session = open(local, remote);
        let id = session
            .edit(|vault| vault.add(note("nouvelle"), at(3)))
            .unwrap();
        assert!(session.vault().get(id).is_some());
        assert_eq!(session.status(), &SyncStatus::Synced);
        assert!(!session.has_pending_changes());
        let saved = session.local.file.as_deref().unwrap();
        let published = session.remote.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(saved, "maître")), ["départ", "nouvelle"]);
        assert_eq!(
            titles(&decrypt(published, "maître")),
            ["départ", "nouvelle"]
        );
    }

    #[test]
    fn edit_without_change_writes_nothing() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        let mut session = open(local, remote);
        let unknown = Uuid::nil();
        let result = session.edit(|vault| vault.delete(unknown, at(3))).unwrap();
        assert_eq!(result, Err(VaultError::EntryNotFound(unknown)));
        assert_eq!(session.local.saves, 0);
        assert_eq!(session.remote.pushes, 0);
    }

    #[test]
    fn offline_changes_stay_pending_until_the_next_sync() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        let mut session = open(local, remote);
        session.remote.offline = true;
        session
            .edit(|vault| vault.add(note("hors ligne"), at(3)))
            .unwrap();
        assert!(session.has_pending_changes());
        assert_eq!(session.local.saves, 1);
        assert!(matches!(session.status(), SyncStatus::Offline(_)));

        session.remote.offline = false;
        assert_eq!(session.sync().unwrap(), &SyncStatus::Synced);
        assert!(!session.has_pending_changes());
        let published = session.remote.file.as_deref().unwrap();
        assert_eq!(
            titles(&decrypt(published, "maître")),
            ["départ", "hors ligne"]
        );
    }

    #[test]
    fn concurrent_publication_is_merged_then_retried() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        let mut session = open(local, remote);
        session.remote.concurrent = vec![changed_elsewhere(&file, "concurrente")];
        session
            .edit(|vault| vault.add(note("locale"), at(3)))
            .unwrap();
        assert_eq!(session.status(), &SyncStatus::Synced);
        assert_eq!(titles(session.vault()), ["concurrente", "départ", "locale"]);
        let published = session.remote.file.as_deref().unwrap();
        assert_eq!(
            titles(&decrypt(published, "maître")),
            ["concurrente", "départ", "locale"]
        );
    }

    #[test]
    fn sync_gives_up_after_three_conflicts() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        let mut session = open(local, remote);
        session.remote.concurrent = ["a", "b", "c"]
            .iter()
            .map(|title| changed_elsewhere(&file, title))
            .collect();
        session
            .edit(|vault| vault.add(note("locale"), at(3)))
            .unwrap();
        assert_eq!(session.status(), &SyncStatus::Conflict);
        assert!(session.has_pending_changes());
        assert_eq!(session.remote.pushes, 0);
    }

    #[test]
    fn refused_push_is_reported_as_offline() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        let mut session = open(local, remote);
        session.remote.push_fails = true;
        session
            .edit(|vault| vault.add(note("locale"), at(3)))
            .unwrap();
        assert_eq!(
            session.status(),
            &SyncStatus::Offline(PortError::new("accès refusé"))
        );
        assert!(session.has_pending_changes());
    }

    #[test]
    fn unusable_remote_is_never_overwritten() {
        let (file, _) = initial_file();
        let (foreign, _) = initial_file();
        let (local, mut remote) = stores(&file);
        remote.file = Some(foreign.clone());
        let mut session = open(local, remote);
        assert_eq!(
            session.status(),
            &SyncStatus::Rejected(FileError::Corrupted)
        );
        session
            .edit(|vault| vault.add(note("locale"), at(3)))
            .unwrap();
        assert_eq!(session.remote.file, Some(foreign));
        assert!(session.has_pending_changes());
    }

    #[test]
    fn failed_local_save_restores_the_vault() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        let mut session = open(local, remote);
        session.local.broken = true;
        let result = session.edit(|vault| vault.add(note("perdue"), at(3)));
        assert_eq!(
            result.err(),
            Some(SessionError::Local(PortError::new("disque plein")))
        );
        assert_eq!(titles(session.vault()), ["départ"]);
        assert_eq!(session.remote.pushes, 0);
    }

    #[test]
    fn changed_password_is_saved_and_published() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        let mut session = open(local, remote);
        session
            .change_password(&secret("nouveau"), fast_params(), at(3))
            .unwrap();
        assert_eq!(session.status(), &SyncStatus::Synced);
        let saved = session.local.file.as_deref().unwrap();
        let published = session.remote.file.as_deref().unwrap();
        assert_eq!(titles(&decrypt(saved, "nouveau")), ["départ"]);
        assert_eq!(titles(&decrypt(published, "nouveau")), ["départ"]);
    }

    fn create(
        local: FakeLocal,
        remote: FakeRemote,
    ) -> Result<(Session<FakeLocal, FakeRemote>, RecoveryKey), SessionError> {
        Session::create(local, remote, &secret("maître"), fast_params(), at(0))
    }

    fn join(
        local: FakeLocal,
        remote: FakeRemote,
        password: &str,
    ) -> Result<Session<FakeLocal, FakeRemote>, SessionError> {
        Session::join(local, remote, Credentials::Password(&secret(password)))
    }

    #[test]
    fn create_saves_then_publishes_an_empty_vault() {
        let (session, recovery_key) = create(FakeLocal::default(), FakeRemote::default()).unwrap();
        assert_eq!(session.vault().entries().count(), 0);
        assert_eq!(session.status(), &SyncStatus::Synced);
        assert!(!session.has_pending_changes());
        let saved = session.local.file.as_deref().unwrap();
        let published = session.remote.file.as_deref().unwrap();
        assert_eq!(decrypt(saved, "maître").id(), session.vault().id());
        assert_eq!(decrypt(published, "maître").id(), session.vault().id());
        let (_, recovered) = VaultFile::unlock_with_recovery_key(published, &recovery_key).unwrap();
        assert_eq!(recovered.id(), session.vault().id());
    }

    #[test]
    fn create_never_replaces_an_existing_vault() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        assert_eq!(
            create(local, FakeRemote::default()).err(),
            Some(SessionError::LocalVaultExists)
        );
        assert_eq!(
            create(FakeLocal::default(), remote).err(),
            Some(SessionError::RemoteVaultExists)
        );
    }

    #[test]
    fn create_requires_a_reachable_remote() {
        let remote = FakeRemote {
            offline: true,
            ..FakeRemote::default()
        };
        assert_eq!(
            create(FakeLocal::default(), remote).err(),
            Some(SessionError::Remote(PortError::new("hors ligne")))
        );
    }

    #[test]
    fn create_keeps_the_vault_pending_if_publication_fails() {
        let remote = FakeRemote {
            push_fails: true,
            ..FakeRemote::default()
        };
        let (session, _) = create(FakeLocal::default(), remote).unwrap();
        assert!(session.has_pending_changes());
        assert_eq!(session.local.saves, 1);
        assert_eq!(session.remote.file, None);
    }

    #[test]
    fn create_reports_a_local_write_failure() {
        assert_eq!(
            Session::create(
                BrokenOnSave(FakeLocal::default()),
                FakeRemote::default(),
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
    fn join_copies_the_remote_vault_locally() {
        let (file, _) = initial_file();
        let (_, remote) = stores(&file);
        let session = join(FakeLocal::default(), remote, "maître").unwrap();
        assert_eq!(titles(session.vault()), ["départ"]);
        assert_eq!(session.status(), &SyncStatus::Synced);
        assert_eq!(session.local.file.as_deref(), Some(file.as_slice()));
        assert_eq!(session.remote.pushes, 0);
    }

    #[test]
    fn join_with_the_recovery_key() {
        let (file, recovery_key) = initial_file();
        let (_, remote) = stores(&file);
        let session = Session::join(
            FakeLocal::default(),
            remote,
            Credentials::RecoveryKey(&recovery_key),
        )
        .unwrap();
        assert_eq!(titles(session.vault()), ["départ"]);
    }

    #[test]
    fn join_never_replaces_a_local_vault() {
        let (file, _) = initial_file();
        let (local, remote) = stores(&file);
        assert_eq!(
            join(local, remote, "maître").err(),
            Some(SessionError::LocalVaultExists)
        );
    }

    #[test]
    fn join_requires_a_remote_vault() {
        assert_eq!(
            join(FakeLocal::default(), FakeRemote::default(), "maître").err(),
            Some(SessionError::RemoteEmpty)
        );
        let offline = FakeRemote {
            offline: true,
            ..FakeRemote::default()
        };
        assert_eq!(
            join(FakeLocal::default(), offline, "maître").err(),
            Some(SessionError::Remote(PortError::new("hors ligne")))
        );
    }

    #[test]
    fn join_with_a_wrong_password_writes_nothing() {
        let (file, _) = initial_file();
        let (_, remote) = stores(&file);
        assert_eq!(
            join(FakeLocal::default(), remote, "autre").err(),
            Some(SessionError::File(FileError::WrongPassword))
        );
    }

    #[test]
    fn join_reports_a_local_write_failure() {
        let (file, _) = initial_file();
        let (_, remote) = stores(&file);
        let local = BrokenOnSave(FakeLocal::default());
        assert_eq!(
            Session::join(local, remote, Credentials::Password(&secret("maître"))).err(),
            Some(SessionError::Local(PortError::new("disque plein")))
        );
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                SessionError::NoLocalVault,
                "aucun coffre local : utiliser init ou join",
            ),
            (
                SessionError::LocalVaultExists,
                "un coffre existe déjà à cet emplacement local",
            ),
            (
                SessionError::RemoteVaultExists,
                "le dépôt distant contient déjà un coffre : utiliser join",
            ),
            (
                SessionError::RemoteEmpty,
                "le dépôt distant ne contient aucun coffre : utiliser init",
            ),
            (
                SessionError::Remote(PortError::new("hors ligne")),
                "dépôt distant : hors ligne",
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
