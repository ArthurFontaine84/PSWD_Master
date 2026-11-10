use std::fs;
use std::path::{Path, PathBuf};

use pswd_core::crypto::KdfParams;
use pswd_core::ports::LocalStore;
use pswd_core::{
    BackupFile, Credentials, RecoveryKey, Secret, Session, SessionError, Timestamp, VaultFile,
};
use pswd_infra::{Config, ConfigFile, FileStore, Locations, RemoteLink, VaultConfig};
use uuid::Uuid;

use crate::password::check_master_password;
use crate::utc::backup_day;
use crate::{AppError, VaultSession};

/// Les coffres de cet appareil, enregistrés dans la configuration.
#[derive(Debug, Clone)]
pub struct Vaults {
    pub(crate) locations: Locations,
    /// Identifiant du dossier d'un nouveau coffre.
    folder_id: fn() -> Uuid,
}

impl Vaults {
    #[must_use]
    pub fn new(locations: Locations) -> Self {
        Self {
            locations,
            folder_id: Uuid::new_v4,
        }
    }

    /// Dossier d'un nouveau coffre : le nom du coffre n'y apparaît pas, il peut donc changer.
    fn new_vault_dir(&self) -> PathBuf {
        self.locations.vault_dir((self.folder_id)())
    }

    pub(crate) fn config_file(&self) -> ConfigFile {
        ConfigFile::new(self.locations.config_file())
    }

    /// Réglages et coffres enregistrés.
    ///
    /// # Errors
    ///
    /// Si la configuration existe mais ne peut pas être lue.
    pub fn config(&self) -> Result<Config, AppError> {
        Ok(self.config_file().load()?)
    }

    /// Modifie les réglages et les enregistre ; renvoie la configuration enregistrée.
    ///
    /// # Errors
    ///
    /// Si la configuration ne peut pas être lue ou écrite.
    pub fn update_config(&self, change: impl FnOnce(&mut Config)) -> Result<Config, AppError> {
        let mut config_file = self.config_file();
        let mut config = config_file.load()?;
        change(&mut config);
        config_file.save(&config)?;
        Ok(config)
    }

    /// Vérifie, avant de demander quoi que ce soit, qu'un coffre peut être créé sous ce nom.
    ///
    /// # Errors
    ///
    /// Si le nom est invalide ou déjà pris.
    pub fn check_new_name(&self, name: &str) -> Result<(), AppError> {
        Ok(self.config()?.ensure_available(name)?)
    }

    /// Crée un coffre privé vide et l'enregistre. Renvoie aussi sa clé de récupération, à
    /// montrer une seule fois. Un essai raté ne laisse rien derrière lui.
    ///
    /// # Errors
    ///
    /// Si le mot de passe est trop faible, si le nom est invalide ou pris, ou si l'écriture
    /// échoue.
    pub fn create(
        &self,
        name: &str,
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
    ) -> Result<(VaultSession, RecoveryKey), AppError> {
        self.create_vault(name, password, params, now, false)
    }

    /// Comme [`Vaults::create`], pour un coffre commun : il pourra être partagé avec d'autres
    /// comptes du serveur une fois publié.
    ///
    /// # Errors
    ///
    /// Comme [`Vaults::create`].
    pub fn create_shared(
        &self,
        name: &str,
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
    ) -> Result<(VaultSession, RecoveryKey), AppError> {
        self.create_vault(name, password, params, now, true)
    }

    fn create_vault(
        &self,
        name: &str,
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
        shared: bool,
    ) -> Result<(VaultSession, RecoveryKey), AppError> {
        let mut config_file = self.config_file();
        let mut config = config_file.load()?;
        config.ensure_available(name)?;
        check_master_password(password)?;
        let vault = VaultConfig {
            path: self.new_vault_dir(),
            shared,
            remote: None,
        };
        let created = removing_on_failure(&vault.path, || {
            let local = FileStore::new(vault.vault_file());
            Ok(Session::create(local, password, params, now)?)
        })?;
        config.add_vault(name, vault)?;
        config_file.save(&config)?;
        Ok(created)
    }

    /// # Errors
    ///
    /// Si le coffre est inconnu, illisible, ou si les identifiants sont faux.
    pub fn open(&self, name: &str, credentials: Credentials<'_>) -> Result<VaultSession, AppError> {
        let local = FileStore::new(self.config()?.vault(name)?.vault_file());
        Ok(Session::open(local, credentials)?)
    }

    /// Supprime le coffre de cet appareil, sans confirmation : c'est à l'interface de la
    /// demander. Les fichiers sont supprimés avant la configuration : un échec ne change rien.
    ///
    /// # Errors
    ///
    /// Si le coffre est inconnu ou si ses fichiers ne peuvent pas être supprimés.
    pub fn delete(&self, name: &str) -> Result<(), AppError> {
        let mut config_file = self.config_file();
        let mut config = config_file.load()?;
        let vault = config.vault(name)?.clone();
        FileStore::new(vault.vault_file()).remove()?;
        // Le dossier n'est retiré que s'il est vide : un fichier inconnu n'est jamais effacé.
        let _ = fs::remove_dir(&vault.path);
        config.remove_vault(name)?;
        config_file.save(&config)?;
        Ok(())
    }

    /// Restaure `file`, le coffre `name` d'une sauvegarde du `date`, comme nouveau coffre :
    /// sous son nom s'il est libre, sinon sous `<name>-sauvegarde-<date>`. Rien n'est jamais
    /// écrasé ni fusionné. Renvoie le nom retenu.
    ///
    /// # Errors
    ///
    /// Si le fichier n'est pas un coffre, si les deux noms sont pris, ou si l'écriture échoue.
    pub fn restore(&self, name: &str, file: &[u8], date: &str) -> Result<String, AppError> {
        VaultFile::check_format(file).map_err(AppError::InvalidBackup)?;
        let config = self.config()?;
        let target = if config.ensure_available(name).is_ok() {
            name.to_owned()
        } else {
            format!("{name}-sauvegarde-{date}")
        };
        self.add_vault_file(&target, file, false, None)?;
        Ok(target)
    }

    /// Enregistre `file` comme nouveau coffre `name`, sans jamais remplacer un fichier.
    pub(crate) fn add_vault_file(
        &self,
        name: &str,
        file: &[u8],
        shared: bool,
        remote: Option<RemoteLink>,
    ) -> Result<(), AppError> {
        let mut config_file = self.config_file();
        let mut config = config_file.load()?;
        config.ensure_available(name)?;
        let vault = VaultConfig {
            path: self.new_vault_dir(),
            shared,
            remote,
        };
        removing_on_failure(&vault.path, || {
            let mut local = FileStore::new(vault.vault_file());
            if local.load()?.is_some() {
                return Err(SessionError::LocalVaultExists.into());
            }
            Ok(local.save_atomic(file)?)
        })?;
        config.add_vault(name, vault)?;
        config_file.save(&config)?;
        Ok(())
    }
}

impl Vaults {
    /// Sauvegarde tous les coffres, ou seulement ceux de `only`, dans un fichier `.pswd` (§7) ;
    /// renvoie ses octets et le nombre de coffres.
    ///
    /// # Errors
    ///
    /// S'il n'y a aucun coffre à sauvegarder, si un nom de `only` est inconnu, ou si le fichier
    /// d'un coffre est absent ou illisible.
    pub fn backup_to_file(
        &self,
        only: Option<&[&str]>,
        now: Timestamp,
    ) -> Result<(Vec<u8>, usize), AppError> {
        let config = self.config()?;
        let vaults: Vec<(&str, &VaultConfig)> = match only {
            Some(names) => names
                .iter()
                .map(|&name| Ok((name, config.vault(name)?)))
                .collect::<Result<_, AppError>>()?,
            None => config
                .vaults
                .iter()
                .map(|(name, vault)| (name.as_str(), vault))
                .collect(),
        };
        if vaults.is_empty() {
            return Err(AppError::NothingToBackUp);
        }
        let mut files = Vec::new();
        for (name, vault) in vaults {
            files.push((name, read_vault_file(name, vault)?));
        }
        let mut backup = BackupFile::new(now);
        for (name, file) in &files {
            backup.add(name, file).map_err(AppError::BackupFile)?;
        }
        Ok((backup.encode(), files.len()))
    }

    /// Coffres contenus dans un fichier `.pswd`, triés par nom.
    ///
    /// # Errors
    ///
    /// [`AppError::EmptyBackup`] s'il n'en contient aucun, ou si ce n'est pas une sauvegarde.
    pub fn backup_file_contents(&self, data: &[u8]) -> Result<Vec<String>, AppError> {
        let backup = BackupFile::parse(data).map_err(AppError::BackupFile)?;
        let names: Vec<String> = backup.names().map(str::to_owned).collect();
        if names.is_empty() {
            return Err(AppError::EmptyBackup);
        }
        Ok(names)
    }

    /// Restaure le coffre `name` d'un fichier `.pswd` (§7) ; renvoie le nom retenu.
    ///
    /// # Errors
    ///
    /// [`AppError::NotInBackup`] si le fichier ne contient pas ce coffre, sinon comme
    /// [`Vaults::restore`].
    pub fn restore_from_file(&self, data: &[u8], name: &str) -> Result<String, AppError> {
        let backup = BackupFile::parse(data).map_err(AppError::BackupFile)?;
        let file = backup
            .vault(name)
            .ok_or_else(|| AppError::NotInBackup(name.to_owned()))?;
        self.restore(name, file, &backup_day(backup.created_at()))
    }
}

/// Fichier `vault.enc` du coffre, qui doit exister.
pub(crate) fn read_vault_file(name: &str, vault: &VaultConfig) -> Result<Vec<u8>, AppError> {
    let path = vault.vault_file();
    FileStore::new(&path).load()?.ok_or_else(|| {
        AppError::File(format!(
            "fichier du coffre {name} introuvable : {}",
            path.display()
        ))
    })
}

/// Supprime le dossier du coffre si `create` échoue alors qu'il n'existait pas avant.
fn removing_on_failure<R>(
    dir: &Path,
    create: impl FnOnce() -> Result<R, AppError>,
) -> Result<R, AppError> {
    let existed = dir.exists();
    let result = create();
    if result.is_err() && !existed {
        // Au mieux : l'erreur de création est plus utile que celle du nettoyage.
        let _ = fs::remove_dir_all(dir);
    }
    result
}

#[cfg(test)]
mod tests {
    use pswd_core::ports::PortError;
    use pswd_core::{BackupFileError, EntryContent, EntryKind, FileError};
    use std::path::PathBuf;

    use pswd_infra::ConfigError;
    use tempfile::TempDir;

    use super::*;

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    fn fast() -> KdfParams {
        KdfParams::new(8 * 1024, 1, 1).unwrap()
    }

    /// Les coffres d'un appareil `device`, dans un dossier temporaire.
    fn device(dir: &TempDir, device: &str) -> Vaults {
        let root = dir.path().join(device);
        Vaults::new(Locations::new(&root.join("config"), &root.join("data")))
    }

    /// Comme [`device`], mais chaque nouveau coffre y reçoit le même dossier, déjà connu.
    fn device_with_known_folder(dir: &TempDir, name: &str) -> (Vaults, PathBuf) {
        let vaults = Vaults {
            folder_id: Uuid::nil,
            ..device(dir, name)
        };
        let folder = vaults.locations.vault_dir(Uuid::nil());
        (vaults, folder)
    }

    fn create(vaults: &Vaults, name: &str) -> VaultSession {
        vaults
            .create(name, &secret("Maître-2026!"), fast(), at(0))
            .unwrap()
            .0
    }

    fn open(vaults: &Vaults, name: &str) -> Result<VaultSession, AppError> {
        vaults.open(name, Credentials::Password(&secret("Maître-2026!")))
    }

    fn names(vaults: &Vaults) -> Vec<String> {
        vaults.config().unwrap().vaults.into_keys().collect()
    }

    fn vault_file(vaults: &Vaults, name: &str) -> PathBuf {
        vaults.config().unwrap().vault(name).unwrap().vault_file()
    }

    #[test]
    fn created_vault_is_registered_and_opens_with_its_keys() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        let (_, recovery_key) = vaults
            .create("perso", &secret("Maître-2026!"), fast(), at(0))
            .unwrap();
        assert_eq!(names(&vaults), ["perso"]);
        let path = vaults
            .config()
            .unwrap()
            .vault("perso")
            .unwrap()
            .path
            .clone();
        assert_eq!(
            path.parent().unwrap(),
            dir.path().join("a").join("data").join("vaults")
        );
        let folder = path.file_name().unwrap().to_str().unwrap();
        assert!(Uuid::try_parse(folder).is_ok(), "{folder}");
        assert!(open(&vaults, "perso").is_ok());
        assert!(
            vaults
                .open("perso", Credentials::RecoveryKey(&recovery_key))
                .is_ok()
        );
        assert_eq!(
            vaults
                .open("perso", Credentials::Password(&secret("autre")))
                .err(),
            Some(AppError::Session(SessionError::File(
                FileError::WrongPassword
            )))
        );
    }

    #[test]
    fn creation_checks_the_name_and_the_password() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        assert_eq!(vaults.check_new_name("perso"), Ok(()));
        create(&vaults, "perso");
        assert_eq!(
            vaults.check_new_name("perso"),
            Err(AppError::Config(ConfigError::VaultExists(
                "perso".to_owned()
            )))
        );
        assert_eq!(
            vaults.create("perso", &secret("x"), fast(), at(0)).err(),
            Some(AppError::Config(ConfigError::VaultExists(
                "perso".to_owned()
            )))
        );
        assert_eq!(
            vaults
                .create("travail", &secret("maître"), fast(), at(0))
                .err(),
            Some(AppError::PasswordTooShort)
        );
        assert_eq!(
            vaults
                .create("travail", &secret("mot de passe maître"), fast(), at(0))
                .err(),
            Some(AppError::PasswordTooSimple)
        );
        assert_eq!(names(&vaults), ["perso"]);
    }

    #[test]
    fn names_are_free_and_each_vault_has_its_own_folder() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        for name in ["Mon coffre", "Été", "Ete", "été"] {
            create(&vaults, name);
        }
        assert_eq!(names(&vaults), ["Ete", "Mon coffre", "Été", "été"]);
        assert!(open(&vaults, "Été").is_ok());
        let config = vaults.config().unwrap();
        let mut folders: Vec<_> = config.vaults.values().map(|vault| &vault.path).collect();
        folders.sort();
        folders.dedup();
        assert_eq!(folders.len(), 4);
        assert_eq!(
            vaults.check_new_name("Été "),
            Err(AppError::Config(ConfigError::InvalidVaultName(
                "Été ".to_owned()
            )))
        );
    }

    #[test]
    fn unregistered_vault_file_is_never_replaced() {
        let dir = TempDir::new().unwrap();
        let (vaults, folder) = device_with_known_folder(&dir, "a");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("vault.enc"), b"ancien").unwrap();
        assert_eq!(
            vaults
                .create("perso", &secret("Maître-2026!"), fast(), at(0))
                .err(),
            Some(AppError::Session(SessionError::LocalVaultExists))
        );
        assert_eq!(fs::read(folder.join("vault.enc")).unwrap(), b"ancien");
        assert_eq!(names(&vaults), Vec::<String>::new());
    }

    #[test]
    fn failed_creation_leaves_nothing_behind() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        let data = dir.path().join("a").join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("vaults"), b"").unwrap();
        let result = vaults.create("perso", &secret("Maître-2026!"), fast(), at(0));
        assert!(
            matches!(
                result,
                Err(AppError::Session(SessionError::Local(PortError { .. })))
            ),
            "{:?}",
            result.err()
        );
        assert_eq!(names(&vaults), Vec::<String>::new());
    }

    #[test]
    fn unknown_vault_cannot_be_opened() {
        let dir = TempDir::new().unwrap();
        assert_eq!(
            open(&device(&dir, "a"), "perso").err(),
            Some(AppError::Config(ConfigError::UnknownVault(
                "perso".to_owned()
            )))
        );
    }

    #[test]
    fn deletion_removes_the_vault_and_frees_its_name() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        create(&vaults, "perso");
        create(&vaults, "travail");
        let folder = vault_file(&vaults, "perso");
        let folder = folder.parent().unwrap().to_owned();
        vaults.delete("perso").unwrap();
        assert_eq!(names(&vaults), ["travail"]);
        assert!(!folder.exists());
        create(&vaults, "perso");
        assert_eq!(
            vaults.delete("inconnu"),
            Err(AppError::Config(ConfigError::UnknownVault(
                "inconnu".to_owned()
            )))
        );
    }

    #[test]
    fn deletion_keeps_unknown_files() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        create(&vaults, "perso");
        let file = vault_file(&vaults, "perso");
        let notes = file.with_file_name("notes.txt");
        fs::write(&notes, b"a garder").unwrap();
        vaults.delete("perso").unwrap();
        assert!(!file.exists());
        assert_eq!(fs::read(&notes).unwrap(), b"a garder");
    }

    #[test]
    fn failed_deletion_changes_nothing() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        create(&vaults, "perso");
        let file = vault_file(&vaults, "perso");
        fs::remove_file(&file).unwrap();
        fs::create_dir(&file).unwrap();
        let error = vaults.delete("perso").unwrap_err();
        assert!(error.to_string().starts_with("suppression de "), "{error}");
        assert_eq!(names(&vaults), ["perso"]);
    }

    /// Le fichier du coffre `perso` de l'appareil `a`, avec une entrée.
    fn saved_vault(dir: &TempDir) -> Vec<u8> {
        let vaults = device(dir, "a");
        let mut session = create(&vaults, "perso");
        session
            .edit(|vault| {
                vault.add(
                    EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
                    at(1),
                )
            })
            .unwrap();
        fs::read(vault_file(&vaults, "perso")).unwrap()
    }

    #[test]
    fn restored_vault_keeps_its_name_when_free() {
        let dir = TempDir::new().unwrap();
        let file = saved_vault(&dir);
        let other = device(&dir, "b");
        assert_eq!(
            other.restore("perso", &file, "2026-10-23"),
            Ok("perso".to_owned())
        );
        assert_eq!(fs::read(vault_file(&other, "perso")).unwrap(), file);
        assert_eq!(open(&other, "perso").unwrap().vault().entries().count(), 1);
    }

    #[test]
    fn restore_never_replaces_an_existing_vault() {
        let dir = TempDir::new().unwrap();
        let file = saved_vault(&dir);
        let vaults = device(&dir, "a");
        let restored = vaults.restore("perso", &file, "2026-10-23").unwrap();
        assert_eq!(restored, "perso-sauvegarde-2026-10-23");
        assert_eq!(names(&vaults), ["perso", "perso-sauvegarde-2026-10-23"]);
        assert_eq!(
            vaults.restore("perso", &file, "2026-10-23"),
            Err(AppError::Config(ConfigError::VaultExists(
                "perso-sauvegarde-2026-10-23".to_owned()
            )))
        );
    }

    #[test]
    fn invalid_file_is_not_restored() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        assert_eq!(
            vaults.restore("perso", b"pas un coffre", "2026-10-23"),
            Err(AppError::InvalidBackup(FileError::Malformed))
        );
        assert_eq!(names(&vaults), Vec::<String>::new());
    }

    #[test]
    fn restore_never_replaces_an_unregistered_vault_file() {
        let dir = TempDir::new().unwrap();
        let file = saved_vault(&dir);
        let (vaults, folder) = device_with_known_folder(&dir, "b");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("vault.enc"), b"ancien").unwrap();
        assert_eq!(
            vaults.restore("perso", &file, "2026-10-23"),
            Err(AppError::Session(SessionError::LocalVaultExists))
        );
        assert_eq!(fs::read(folder.join("vault.enc")).unwrap(), b"ancien");
        assert_eq!(names(&vaults), Vec::<String>::new());
    }

    #[test]
    fn failed_restore_leaves_nothing_behind() {
        let dir = TempDir::new().unwrap();
        let file = saved_vault(&dir);
        let vaults = device(&dir, "b");
        let data = dir.path().join("b").join("data");
        fs::create_dir_all(&data).unwrap();
        fs::write(data.join("vaults"), b"").unwrap();
        assert!(matches!(
            vaults.restore("perso", &file, "2026-10-23"),
            Err(AppError::Port(_))
        ));
        assert_eq!(names(&vaults), Vec::<String>::new());
    }

    fn lock_after_three_minutes(config: &mut Config) {
        config.auto_lock_minutes = 3;
    }

    #[test]
    fn settings_are_changed_and_saved() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        create(&vaults, "perso");
        let saved = vaults.update_config(lock_after_three_minutes).unwrap();
        assert_eq!(saved.auto_lock_minutes, 3);
        assert_eq!(vaults.config().unwrap().auto_lock_minutes, 3);
        assert_eq!(names(&vaults), ["perso"]);
    }

    #[test]
    fn unreadable_configuration_is_not_overwritten() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        let file = dir.path().join("a").join("config").join("config.toml");
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "couleur = 1").unwrap();
        assert!(matches!(
            vaults.update_config(lock_after_three_minutes),
            Err(AppError::Config(ConfigError::Invalid(_)))
        ));
        assert_eq!(fs::read_to_string(&file).unwrap(), "couleur = 1");
    }

    #[test]
    fn a_backup_file_is_listed_then_restored() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        create(&vaults, "travail");
        create(&vaults, "perso");
        let (data, count) = vaults.backup_to_file(None, at(1_761_229_809_999)).unwrap();
        assert_eq!(count, 2);
        let other = device(&dir, "b");
        assert_eq!(
            other.backup_file_contents(&data),
            Ok(vec!["perso".to_owned(), "travail".to_owned()])
        );
        assert_eq!(
            other.restore_from_file(&data, "perso"),
            Ok("perso".to_owned())
        );
        assert!(open(&other, "perso").is_ok());
        assert_eq!(
            vaults.restore_from_file(&data, "perso"),
            Ok("perso-sauvegarde-2025-10-23".to_owned())
        );
        assert_eq!(
            other.restore_from_file(&data, "autre"),
            Err(AppError::NotInBackup("autre".to_owned()))
        );
        assert_eq!(names(&other), ["perso"]);
    }

    #[test]
    fn a_backup_file_can_hold_some_vaults_only() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        create(&vaults, "perso");
        create(&vaults, "travail");
        create(&vaults, "famille");
        let (data, count) = vaults
            .backup_to_file(Some(&["travail", "famille"]), at(0))
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(
            vaults.backup_file_contents(&data),
            Ok(vec!["famille".to_owned(), "travail".to_owned()])
        );
    }

    #[test]
    fn a_backup_file_needs_known_vaults() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        assert_eq!(
            vaults.backup_to_file(None, at(0)),
            Err(AppError::NothingToBackUp)
        );
        create(&vaults, "perso");
        assert_eq!(
            vaults.backup_to_file(Some(&[]), at(0)),
            Err(AppError::NothingToBackUp)
        );
        assert_eq!(
            vaults.backup_to_file(Some(&["perso", "travail"]), at(0)),
            Err(AppError::Config(ConfigError::UnknownVault(
                "travail".to_owned()
            )))
        );
    }

    #[test]
    fn a_missing_or_damaged_vault_stops_the_backup_file() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        create(&vaults, "perso");
        let file = vault_file(&vaults, "perso");
        fs::write(&file, "abîmé").unwrap();
        assert_eq!(
            vaults.backup_to_file(None, at(0)),
            Err(AppError::BackupFile(BackupFileError::InvalidVault(
                "perso".to_owned(),
                FileError::Malformed
            )))
        );
        fs::remove_file(&file).unwrap();
        assert_eq!(
            vaults.backup_to_file(None, at(0)),
            Err(AppError::File(format!(
                "fichier du coffre perso introuvable : {}",
                file.display()
            )))
        );
    }

    #[test]
    fn an_empty_or_foreign_file_is_not_a_backup() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        let empty = BackupFile::new(at(0)).encode();
        assert_eq!(
            vaults.backup_file_contents(&empty),
            Err(AppError::EmptyBackup)
        );
        let foreign = Some(AppError::BackupFile(BackupFileError::Malformed));
        assert_eq!(vaults.backup_file_contents(b"PSWD").err(), foreign);
        assert_eq!(vaults.restore_from_file(b"PSWD", "perso").err(), foreign);
        assert_eq!(names(&vaults), Vec::<String>::new());
    }
}
