use std::fs;
use std::path::{Path, PathBuf};

use pswd_core::crypto::KdfParams;
use pswd_core::ports::LocalStore;
use pswd_core::{Credentials, RecoveryKey, Secret, Session, SessionError, Timestamp, VaultFile};
use pswd_infra::{Config, ConfigFile, FileStore, Locations, VaultConfig, create_backup};

use crate::utc::backup_stamp;
use crate::{AppError, VaultSession};

/// Les coffres de cet appareil, enregistrés dans la configuration.
#[derive(Debug, Clone)]
pub struct Vaults {
    locations: Locations,
}

impl Vaults {
    #[must_use]
    pub fn new(locations: Locations) -> Self {
        Self { locations }
    }

    fn config_file(&self) -> ConfigFile {
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

    /// Vérifie, avant de demander quoi que ce soit, qu'un coffre peut être créé sous ce nom.
    ///
    /// # Errors
    ///
    /// Si le nom est invalide ou déjà pris.
    pub fn check_new_name(&self, name: &str) -> Result<(), AppError> {
        Ok(self.config()?.ensure_available(name)?)
    }

    /// Crée un coffre vide et l'enregistre. Renvoie aussi sa clé de récupération, à montrer une
    /// seule fois. Un essai raté ne laisse rien derrière lui.
    ///
    /// # Errors
    ///
    /// Si le mot de passe est vide, si le nom est invalide ou pris, ou si l'écriture échoue.
    pub fn create(
        &self,
        name: &str,
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
    ) -> Result<(VaultSession, RecoveryKey), AppError> {
        if password.expose().is_empty() {
            return Err(AppError::EmptyPassword);
        }
        let mut config_file = self.config_file();
        let mut config = config_file.load()?;
        config.ensure_available(name)?;
        let vault = VaultConfig {
            path: self.locations.vault_dir(name),
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

    /// Sauvegarde tous les coffres, ou seulement `only`, dans un nouveau dossier daté de
    /// `destination`. Renvoie ce dossier et le nombre de coffres sauvegardés.
    ///
    /// # Errors
    ///
    /// S'il n'y a aucun coffre, si `only` est inconnu, ou si la copie échoue.
    pub fn backup(
        &self,
        destination: &Path,
        only: Option<&str>,
        now: Timestamp,
    ) -> Result<(PathBuf, usize), AppError> {
        let config = self.config()?;
        let vaults: Vec<(&str, &VaultConfig)> = match only {
            Some(name) => vec![(name, config.vault(name)?)],
            None => config
                .vaults
                .iter()
                .map(|(name, vault)| (name.as_str(), vault))
                .collect(),
        };
        if vaults.is_empty() {
            return Err(AppError::NothingToBackUp);
        }
        let backup = create_backup(destination, &backup_stamp(now), &vaults)?;
        Ok((backup, vaults.len()))
    }

    /// Restaure `file`, le coffre `name` d'une sauvegarde datée du `date`, comme nouveau coffre :
    /// sous son nom s'il est libre, sinon sous `<name>-sauvegarde-<date>`. Rien n'est jamais
    /// écrasé ni fusionné. Renvoie le nom retenu.
    ///
    /// # Errors
    ///
    /// Si le fichier n'est pas un coffre, si les deux noms sont pris, ou si l'écriture échoue.
    pub fn restore(&self, name: &str, file: &[u8], date: &str) -> Result<String, AppError> {
        VaultFile::check_format(file).map_err(AppError::InvalidBackup)?;
        let mut config_file = self.config_file();
        let mut config = config_file.load()?;
        let target = if config.ensure_available(name).is_ok() {
            name.to_owned()
        } else {
            format!("{name}-sauvegarde-{date}")
        };
        config.ensure_available(&target)?;
        let vault = VaultConfig {
            path: self.locations.vault_dir(&target),
        };
        removing_on_failure(&vault.path, || {
            let mut local = FileStore::new(vault.vault_file());
            if local.load()?.is_some() {
                return Err(SessionError::LocalVaultExists.into());
            }
            Ok(local.save_atomic(file)?)
        })?;
        config.add_vault(&target, vault)?;
        config_file.save(&config)?;
        Ok(target)
    }
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
    use pswd_core::{EntryContent, EntryKind, FileError};
    use pswd_infra::{ConfigError, backup_vault_file, backup_vault_names};
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

    fn create(vaults: &Vaults, name: &str) -> VaultSession {
        vaults
            .create(name, &secret("maître"), fast(), at(0))
            .unwrap()
            .0
    }

    fn open(vaults: &Vaults, name: &str) -> Result<VaultSession, AppError> {
        vaults.open(name, Credentials::Password(&secret("maître")))
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
            .create("perso", &secret("maître"), fast(), at(0))
            .unwrap();
        assert_eq!(names(&vaults), ["perso"]);
        assert_eq!(
            vaults.config().unwrap().vault("perso").unwrap().path,
            dir.path()
                .join("a")
                .join("data")
                .join("vaults")
                .join("perso")
        );
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
            vaults.create("travail", &secret(""), fast(), at(0)).err(),
            Some(AppError::EmptyPassword)
        );
        assert_eq!(names(&vaults), ["perso"]);
    }

    #[test]
    fn unregistered_vault_file_is_never_replaced() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        let folder = dir
            .path()
            .join("a")
            .join("data")
            .join("vaults")
            .join("perso");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("vault.enc"), b"ancien").unwrap();
        assert_eq!(
            vaults
                .create("perso", &secret("maître"), fast(), at(0))
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
        let result = vaults.create("perso", &secret("maître"), fast(), at(0));
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

    /// Dossier qui tient lieu de clé USB.
    fn usb_key(dir: &TempDir) -> PathBuf {
        let usb = dir.path().join("usb");
        fs::create_dir_all(&usb).unwrap();
        usb
    }

    #[test]
    fn backup_copies_every_vault_or_only_one() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        create(&vaults, "perso");
        create(&vaults, "travail");
        let usb = usb_key(&dir);
        let (folder, count) = vaults.backup(&usb, None, at(1_000)).unwrap();
        assert_eq!(
            folder,
            usb.join("pswd-sauvegardes").join("1970-01-01_00-00-01")
        );
        assert_eq!(count, 2);
        assert_eq!(backup_vault_names(&folder).unwrap(), ["perso", "travail"]);
        assert_eq!(
            fs::read(backup_vault_file(&folder, "perso")).unwrap(),
            fs::read(vault_file(&vaults, "perso")).unwrap()
        );
        let (folder, count) = vaults.backup(&usb, Some("travail"), at(2_000)).unwrap();
        assert_eq!(count, 1);
        assert_eq!(backup_vault_names(&folder).unwrap(), ["travail"]);
    }

    #[test]
    fn backup_requires_a_known_vault() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        let usb = usb_key(&dir);
        assert_eq!(
            vaults.backup(&usb, None, at(0)).err(),
            Some(AppError::NothingToBackUp)
        );
        assert_eq!(
            vaults.backup(&usb, Some("perso"), at(0)).err(),
            Some(AppError::Config(ConfigError::UnknownVault(
                "perso".to_owned()
            )))
        );
    }

    #[test]
    fn backup_to_a_missing_folder_is_refused() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        create(&vaults, "perso");
        let error = vaults
            .backup(&dir.path().join("débranchée"), None, at(0))
            .unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("dossier de destination introuvable"),
            "{error}"
        );
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
        let vaults = device(&dir, "b");
        let folder = dir
            .path()
            .join("b")
            .join("data")
            .join("vaults")
            .join("perso");
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
}
