//! Ce qui se fait sur la machine du serveur : configuration, sauvegarde et restauration de la
//! base (SPEC §11).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use pswd_core::{Timestamp, server_backup_file_name};
use serde::Deserialize;

use crate::storage::{Storage, StorageError};

/// Nom de la base dans le dossier des données.
pub const DATABASE_FILE: &str = "pswd.db";

/// Port d'écoute quand la configuration n'en donne pas.
pub const DEFAULT_PORT: u16 = 8443;

/// `/etc/pswd-server/config.toml` ; absent, les valeurs par défaut s'appliquent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerSettings {
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self { port: DEFAULT_PORT }
    }
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

#[derive(Debug)]
pub enum MaintenanceError {
    /// La configuration existe mais ne peut pas être lue.
    Config(String),
    FolderMissing(PathBuf),
    NoDatabase(PathBuf),
    /// Un fichier existe déjà là où il serait écrit : rien n'est écrasé.
    AlreadyExists(PathBuf),
    /// Le serveur a déjà des comptes ou des coffres : il n'est pas neuf.
    NotFresh,
    NotABackup(PathBuf),
    /// Chemin qui n'est pas de l'UTF-8 : SQLite ne le recevrait pas tel quel.
    InvalidPath(PathBuf),
    Storage(StorageError),
    Io(io::Error),
}

impl fmt::Display for MaintenanceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) => write!(f, "configuration invalide : {message}"),
            Self::FolderMissing(path) => write!(f, "dossier introuvable : {}", path.display()),
            Self::NoDatabase(path) => write!(f, "aucune base à sauvegarder : {}", path.display()),
            Self::AlreadyExists(path) => write!(f, "le fichier existe déjà : {}", path.display()),
            Self::NotFresh => f.write_str(
                "le serveur a déjà des comptes ou des coffres : la restauration se fait seulement sur un serveur neuf",
            ),
            Self::NotABackup(path) => write!(
                f,
                "ce fichier n'est pas une sauvegarde de pswd-server : {}",
                path.display()
            ),
            Self::InvalidPath(path) => write!(f, "chemin illisible : {}", path.display()),
            Self::Storage(error) => error.fmt(f),
            Self::Io(error) => write!(f, "erreur de fichier : {error}"),
        }
    }
}

impl std::error::Error for MaintenanceError {}

impl From<StorageError> for MaintenanceError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<io::Error> for MaintenanceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Lit la configuration ; un fichier absent donne les valeurs par défaut.
///
/// # Errors
///
/// Si le fichier existe mais ne peut pas être lu, ou n'est pas une configuration valide.
pub fn load_settings(path: &Path) -> Result<ServerSettings, MaintenanceError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ServerSettings::default());
        }
        Err(error) => return Err(MaintenanceError::Config(error.to_string())),
    };
    toml::from_str(&text).map_err(|error| MaintenanceError::Config(error.message().to_owned()))
}

/// Écrit dans `folder` une copie de la base, `pswd-server-sauvegarde_<date>.db`, sans arrêter le
/// serveur ; renvoie son chemin.
///
/// # Errors
///
/// Si le dossier n'existe pas, s'il n'y a pas de base, si le fichier existe déjà ou si la copie
/// échoue.
pub fn backup(data_dir: &Path, folder: &Path, now: Timestamp) -> Result<PathBuf, MaintenanceError> {
    let database = data_dir.join(DATABASE_FILE);
    if !database.is_file() {
        return Err(MaintenanceError::NoDatabase(database));
    }
    let target = folder.join(server_backup_file_name(now));
    let Some(path) = target.to_str() else {
        return Err(MaintenanceError::InvalidPath(target));
    };
    if !folder.is_dir() {
        return Err(MaintenanceError::FolderMissing(folder.to_owned()));
    }
    if target.exists() {
        return Err(MaintenanceError::AlreadyExists(target));
    }
    Storage::open(&database)?.backup_to(path)?;
    Ok(target)
}

/// Remplace la base d'un serveur neuf (ni compte ni coffre) par la sauvegarde `file`. Le service
/// doit être arrêté. Le certificat restauré garde son empreinte : les appareils se reconnectent
/// sans rien refaire.
///
/// # Errors
///
/// Si le serveur n'est pas neuf, si `file` n'est pas une sauvegarde de `pswd-server`, ou si la
/// copie échoue ; la base en place n'est alors pas touchée.
pub fn restore(data_dir: &Path, file: &Path) -> Result<(), MaintenanceError> {
    if !data_dir.is_dir() {
        return Err(MaintenanceError::FolderMissing(data_dir.to_owned()));
    }
    let database = data_dir.join(DATABASE_FILE);
    if database.exists() && !Storage::open(&database)?.is_fresh()? {
        return Err(MaintenanceError::NotFresh);
    }
    let backup =
        Storage::open_read_only(file).map_err(|_| MaintenanceError::NotABackup(file.to_owned()))?;
    if !matches!(backup.identity(), Ok(Some(_))) {
        return Err(MaintenanceError::NotABackup(file.to_owned()));
    }
    drop(backup);
    let temporary = data_dir.join(format!("{DATABASE_FILE}.restauration"));
    let copied = fs::copy(file, &temporary).and_then(|_| fs::rename(&temporary, &database));
    if let Err(error) = copied {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    adopt_folder_owner(data_dir)
}

/// Donne à la base le propriétaire du dossier des données : une commande lancée avec `sudo`
/// (`reset-admin`, `restore`) ne doit pas laisser une base que le service ne peut plus écrire.
///
/// # Errors
///
/// Si le propriétaire ne peut pas être changé.
#[cfg(unix)]
pub fn adopt_folder_owner(data_dir: &Path) -> Result<(), MaintenanceError> {
    use std::os::unix::fs::{MetadataExt, chown};

    let folder = fs::metadata(data_dir)?;
    let database = data_dir.join(DATABASE_FILE);
    let file = fs::metadata(&database)?;
    if (file.uid(), file.gid()) != (folder.uid(), folder.gid()) {
        chown(&database, Some(folder.uid()), Some(folder.gid()))?;
    }
    Ok(())
}

/// Sous Windows (développement seulement), rien à faire.
///
/// # Errors
///
/// Jamais.
#[cfg(not(unix))]
pub fn adopt_folder_owner(_: &Path) -> Result<(), MaintenanceError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use pswd_core::Timestamp;
    use tempfile::TempDir;

    use super::*;
    use crate::load_or_create_identity;
    use crate::storage::{KeyPurpose, NewDevice};

    const NOW: Timestamp = Timestamp::from_millis(1_761_229_809_999);
    const BACKUP_NAME: &str = "pswd-server-sauvegarde_2025-10-23_14-30-09.db";

    /// Dossier de données avec une base et son certificat ; `with_account` y ajoute un compte.
    fn data_dir(with_account: bool) -> TempDir {
        let dir = TempDir::new().unwrap();
        let mut storage = Storage::open(&dir.path().join(DATABASE_FILE)).unwrap();
        load_or_create_identity(&mut storage).unwrap();
        if with_account {
            let later = Timestamp::from_millis(i64::MAX);
            storage
                .add_connection_key(b"cle", &KeyPurpose::Admin, later)
                .unwrap();
            let device = NewDevice {
                name: "PC",
                token_hash: b"jeton",
                account_name: Some("Arthur"),
            };
            storage.redeem_connection_key(b"cle", NOW, &device).unwrap();
        }
        dir
    }

    fn fingerprint(dir: &Path) -> [u8; 32] {
        let storage = Storage::open(&dir.join(DATABASE_FILE)).unwrap();
        storage.identity().unwrap().unwrap().fingerprint()
    }

    #[test]
    fn settings_default_to_port_8443() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        assert_eq!(load_settings(&path).unwrap().port, DEFAULT_PORT);
        fs::write(&path, "").unwrap();
        assert_eq!(load_settings(&path).unwrap(), ServerSettings::default());
        fs::write(&path, "port = 9000\n").unwrap();
        assert_eq!(load_settings(&path).unwrap().port, 9000);
    }

    #[test]
    fn invalid_settings_are_explained() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        for text in ["port = 70000", "couleur = 1", "port = "] {
            fs::write(&path, text).unwrap();
            let error = load_settings(&path).unwrap_err();
            assert!(
                error.to_string().starts_with("configuration invalide : "),
                "{error}"
            );
        }
        assert!(matches!(
            load_settings(dir.path()),
            Err(MaintenanceError::Config(_))
        ));
    }

    #[test]
    fn a_backup_is_written_once_in_an_existing_folder() {
        let data = data_dir(true);
        let folder = TempDir::new().unwrap();
        let path = backup(data.path(), folder.path(), NOW).unwrap();
        assert_eq!(path, folder.path().join(BACKUP_NAME));
        let copy = Storage::open_read_only(&path).unwrap();
        assert!(!copy.is_fresh().unwrap());

        assert!(matches!(
            backup(data.path(), folder.path(), NOW),
            Err(MaintenanceError::AlreadyExists(_))
        ));
        let missing = folder.path().join("absent");
        assert!(matches!(
            backup(data.path(), &missing, NOW),
            Err(MaintenanceError::FolderMissing(_))
        ));
        assert!(matches!(
            backup(folder.path(), folder.path(), NOW),
            Err(MaintenanceError::NoDatabase(_))
        ));
    }

    /// Chemin que SQLite ne pourrait pas recevoir : ce n'est pas de l'UTF-8.
    #[cfg(unix)]
    fn invalid_path() -> PathBuf {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(b"\xFF"))
    }

    #[cfg(windows)]
    fn invalid_path() -> PathBuf {
        use std::os::windows::ffi::OsStringExt;
        PathBuf::from(std::ffi::OsString::from_wide(&[0xD800]))
    }

    #[test]
    fn a_path_that_is_not_utf8_is_refused() {
        let data = data_dir(false);
        assert!(matches!(
            backup(data.path(), &invalid_path(), NOW),
            Err(MaintenanceError::InvalidPath(_))
        ));
    }

    #[test]
    fn an_unreadable_database_is_reported() {
        let data = TempDir::new().unwrap();
        fs::write(data.path().join(DATABASE_FILE), "pas une base").unwrap();
        let folder = TempDir::new().unwrap();
        assert!(matches!(
            backup(data.path(), folder.path(), NOW),
            Err(MaintenanceError::Storage(_))
        ));
        let file = backup(data_dir(true).path(), folder.path(), NOW).unwrap();
        assert!(matches!(
            restore(data.path(), &file),
            Err(MaintenanceError::Storage(_))
        ));
    }

    #[test]
    fn a_backup_restores_a_fresh_server_with_the_same_certificate() {
        let old = data_dir(true);
        let folder = TempDir::new().unwrap();
        let file = backup(old.path(), folder.path(), NOW).unwrap();

        let empty = TempDir::new().unwrap();
        restore(empty.path(), &file).unwrap();
        assert_eq!(fingerprint(empty.path()), fingerprint(old.path()));

        let fresh = data_dir(false);
        restore(fresh.path(), &file).unwrap();
        assert_eq!(fingerprint(fresh.path()), fingerprint(old.path()));
        assert!(!fresh.path().join("pswd.db.restauration").exists());
    }

    #[test]
    fn a_server_in_use_is_never_replaced() {
        let used = data_dir(true);
        let other = data_dir(true);
        let folder = TempDir::new().unwrap();
        let file = backup(other.path(), folder.path(), NOW).unwrap();
        let error = restore(used.path(), &file).unwrap_err();
        assert!(matches!(error, MaintenanceError::NotFresh));
        assert_ne!(fingerprint(used.path()), fingerprint(other.path()));
    }

    #[test]
    fn only_a_server_backup_is_restored() {
        let fresh = data_dir(false);
        let before = fingerprint(fresh.path());
        let folder = TempDir::new().unwrap();
        let text = folder.path().join("texte.db");
        fs::write(&text, "pas une base").unwrap();
        let without_certificate = folder.path().join("vide.db");
        Storage::open(&without_certificate).unwrap();
        for file in [text, without_certificate] {
            assert!(matches!(
                restore(fresh.path(), &file),
                Err(MaintenanceError::NotABackup(_))
            ));
        }
        assert_eq!(fingerprint(fresh.path()), before);
        let missing = folder.path().join("absent");
        assert!(matches!(
            restore(&missing, &folder.path().join("vide.db")),
            Err(MaintenanceError::FolderMissing(_))
        ));
    }

    #[test]
    fn a_failed_copy_installs_nothing() {
        let old = data_dir(true);
        let folder = TempDir::new().unwrap();
        let file = backup(old.path(), folder.path(), NOW).unwrap();
        let target = TempDir::new().unwrap();
        // Un dossier à la place du fichier temporaire : la copie échoue.
        fs::create_dir(target.path().join("pswd.db.restauration")).unwrap();
        let error = restore(target.path(), &file).unwrap_err();
        assert!(matches!(error, MaintenanceError::Io(_)), "{error}");
        assert!(!target.path().join(DATABASE_FILE).exists());
    }

    #[test]
    fn the_database_keeps_the_owner_of_its_folder() {
        let data = data_dir(false);
        adopt_folder_owner(data.path()).unwrap();
        assert!(data.path().join(DATABASE_FILE).is_file());
    }

    #[test]
    fn errors_have_readable_messages() {
        let path = PathBuf::from("/srv/x");
        let messages = [
            (
                MaintenanceError::FolderMissing(path.clone()),
                "dossier introuvable : ",
            ),
            (
                MaintenanceError::NoDatabase(path.clone()),
                "aucune base à sauvegarder : ",
            ),
            (
                MaintenanceError::AlreadyExists(path.clone()),
                "le fichier existe déjà : ",
            ),
            (
                MaintenanceError::NotFresh,
                "le serveur a déjà des comptes ou des coffres",
            ),
            (
                MaintenanceError::NotABackup(path.clone()),
                "ce fichier n'est pas une sauvegarde",
            ),
            (MaintenanceError::InvalidPath(path), "chemin illisible : "),
            (
                MaintenanceError::Storage(StorageError::NotFound),
                "coffre inconnu",
            ),
            (
                MaintenanceError::Io(io::Error::other("disque plein")),
                "erreur de fichier : disque plein",
            ),
        ];
        for (error, start) in messages {
            assert!(error.to_string().starts_with(start), "{error}");
        }
    }
}
