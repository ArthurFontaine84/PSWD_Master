//! Base SQLite du serveur : coffres opaques et leurs versions, comptes, appareils, certificat.

mod accounts;
mod admin;
mod sharing;
mod vaults;

use std::fmt;
use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use zeroize::Zeroizing;

use crate::tls::Identity;

pub use accounts::{Caller, KeyPurpose, NewDevice};
pub use admin::{
    AccountDetails, DeviceSummary, PendingKey, PendingPurpose, Restored, ServerVault,
    VersionSummary,
};
pub use sharing::{AccountSummary, Members, ReceivedInvitation, SentInvitation};
pub use vaults::{KEPT_VERSIONS, StoredFile, VaultSummary};

/// Version actuelle du schéma, rangée dans `PRAGMA user_version`.
const SCHEMA_VERSION: i64 = 1;

/// Attente maximale d'une base occupée par une autre connexion.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const SCHEMA: &str = "
    CREATE TABLE accounts (
        id BLOB PRIMARY KEY,
        name TEXT NOT NULL,
        is_admin INTEGER NOT NULL,
        created_at INTEGER NOT NULL
    );
    CREATE TABLE devices (
        id BLOB PRIMARY KEY,
        account_id BLOB NOT NULL REFERENCES accounts(id),
        name TEXT NOT NULL,
        token_hash BLOB NOT NULL UNIQUE,
        created_at INTEGER NOT NULL
    );
    -- Compte visé : `account_id` pour un appareil de plus, `account_name` pour un nouveau
    -- membre, aucun des deux pour un administrateur.
    CREATE TABLE connection_keys (
        secret_hash BLOB PRIMARY KEY,
        id BLOB NOT NULL UNIQUE,
        account_id BLOB REFERENCES accounts(id),
        account_name TEXT,
        expires_at INTEGER NOT NULL
    );
    -- Un coffre supprimé (`removed` : 1 par un administrateur, 2 partout par un membre) n'a
    -- plus ni contenu, ni versions, ni membres : il ne reste que la trace de ses anciens
    -- membres, qui reçoivent 410.
    CREATE TABLE vaults (
        id BLOB PRIMARY KEY,
        name TEXT NOT NULL,
        shared INTEGER NOT NULL,
        current_version INTEGER,
        removed INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE vault_members (
        vault_id BLOB NOT NULL REFERENCES vaults(id),
        account_id BLOB NOT NULL REFERENCES accounts(id),
        PRIMARY KEY (vault_id, account_id)
    );
    -- Comptes qui ont quitté le coffre ou en ont été retirés : leurs appareils reçoivent 410.
    CREATE TABLE former_members (
        vault_id BLOB NOT NULL REFERENCES vaults(id),
        account_id BLOB NOT NULL REFERENCES accounts(id),
        PRIMARY KEY (vault_id, account_id)
    );
    CREATE TABLE invitations (
        id BLOB PRIMARY KEY,
        vault_id BLOB NOT NULL REFERENCES vaults(id),
        account_id BLOB NOT NULL REFERENCES accounts(id),
        invited_by BLOB NOT NULL REFERENCES accounts(id),
        created_at INTEGER NOT NULL,
        UNIQUE (vault_id, account_id)
    );
    CREATE TABLE vault_versions (
        vault_id BLOB NOT NULL REFERENCES vaults(id),
        version INTEGER NOT NULL,
        content BLOB NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY (vault_id, version)
    );
    CREATE TABLE identity (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        certificate BLOB NOT NULL,
        private_key BLOB NOT NULL
    );
    CREATE TABLE settings (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        public_address TEXT
    );
    INSERT INTO settings (id) VALUES (1);
";

#[derive(Debug)]
pub enum StorageError {
    /// Coffre ou invitation inconnus, ou dont le compte n'est pas membre.
    NotFound,
    /// Le compte a quitté le coffre commun ou en a été retiré.
    Gone,
    /// Le coffre a été supprimé du serveur par un administrateur.
    Deleted,
    /// Le coffre a été supprimé partout par un de ses membres : ses appareils suppriment leur
    /// copie.
    DeletedEverywhere,
    /// Un coffre privé ne se partage pas et ne se quitte pas.
    NotShared,
    /// Le compte invité n'existe pas.
    UnknownAccount,
    /// Le compte invité est déjà membre ou déjà invité.
    AlreadyMember,
    /// Un coffre commun se quitte : seul son dernier membre le supprime.
    SharedVault,
    /// Seul un coffre sans membre reçoit un compte rattaché par un administrateur.
    HasMembers,
    /// Le dernier administrateur ne perd pas son rôle et n'est pas supprimé.
    LastAdmin,
    /// Un administrateur ne supprime que son propre compte parmi les administrateurs : un autre
    /// perd d'abord son rôle.
    OtherAdmin,
    /// Le coffre a changé depuis la version attendue.
    Conflict,
    /// Clé de connexion inconnue, expirée ou déjà utilisée.
    InvalidKey,
    /// Une clé administrateur crée un compte : il lui faut un nom.
    MissingAccountName,
    /// Le nom d'un nouveau membre est déjà celui d'un compte ou d'un autre nouveau membre.
    NameTaken,
    RandomUnavailable,
    /// Base créée par une version plus récente du serveur.
    UnknownSchema(i64),
    Database(rusqlite::Error),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("coffre inconnu"),
            Self::Gone => f.write_str("coffre retiré"),
            Self::Deleted => f.write_str("coffre supprimé"),
            Self::DeletedEverywhere => f.write_str("coffre supprimé partout"),
            Self::NotShared => f.write_str("coffre privé"),
            Self::UnknownAccount => f.write_str("compte inconnu"),
            Self::AlreadyMember => f.write_str("déjà membre ou déjà invité"),
            Self::SharedVault => f.write_str("coffre commun"),
            Self::HasMembers => f.write_str("le coffre a encore des membres"),
            Self::LastAdmin => f.write_str("dernier administrateur"),
            Self::OtherAdmin => f.write_str("compte d'un autre administrateur"),
            Self::Conflict => f.write_str("le coffre a changé depuis la version lue"),
            Self::InvalidKey => f.write_str("clé de connexion invalide ou expirée"),
            Self::MissingAccountName => f.write_str("nom du compte manquant"),
            Self::NameTaken => f.write_str("nom de compte déjà pris"),
            Self::RandomUnavailable => f.write_str("générateur aléatoire du système indisponible"),
            Self::UnknownSchema(version) => {
                write!(f, "base de données de version inconnue ({version})")
            }
            Self::Database(error) => write!(f, "base de données : {error}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

/// L'aléa du système ne manque que sur une machine défaillante.
// Exclue de la couverture : impossible à provoquer en test.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) fn random_unavailable<E>(_: E) -> StorageError {
    StorageError::RandomUnavailable
}

pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Ouvre la base, ou la crée si le fichier n'existe pas.
    ///
    /// # Errors
    ///
    /// Si le fichier ne peut pas être ouvert ou si son schéma est inconnu.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Self::init(Connection::open(path)?)
    }

    /// # Errors
    ///
    /// Si SQLite ne peut pas créer la base.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        Self::init(Connection::open_in_memory()?)
    }

    /// Ouvre une sauvegarde sans la modifier, après avoir vérifié son schéma.
    ///
    /// # Errors
    ///
    /// Si le fichier n'est pas une base SQLite, ou si son schéma est inconnu.
    pub fn open_read_only(path: &Path) -> Result<Self, StorageError> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version != SCHEMA_VERSION {
            return Err(StorageError::UnknownSchema(version));
        }
        Ok(Self { conn })
    }

    fn init(mut conn: Connection) -> Result<Self, StorageError> {
        // `pswd-server backup` lit la base pendant que le service y écrit.
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.pragma_update(None, "foreign_keys", true)?;
        // Un coffre supprimé est effacé du fichier lui-même, et absent des sauvegardes.
        conn.pragma_update(None, "secure_delete", true)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        match version {
            0 => {
                tx.execute_batch(SCHEMA)?;
                tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            }
            SCHEMA_VERSION => {}
            other => return Err(StorageError::UnknownSchema(other)),
        }
        tx.commit()?;
        Ok(Self { conn })
    }

    /// Certificat du serveur et sa clé privée, s'ils ont déjà été créés.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn identity(&self) -> Result<Option<Identity>, StorageError> {
        let identity = self
            .conn
            .query_row(
                "SELECT certificate, private_key FROM identity WHERE id = 1",
                [],
                |row| {
                    Ok(Identity::from_parts(
                        row.get(0)?,
                        Zeroizing::new(row.get(1)?),
                    ))
                },
            )
            .optional()?;
        Ok(identity)
    }

    /// Enregistre le certificat du serveur ; il n'est jamais remplacé.
    ///
    /// # Errors
    ///
    /// Si un certificat existe déjà ou si l'écriture échoue.
    pub fn save_identity(&mut self, identity: &Identity) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO identity (id, certificate, private_key) VALUES (1, ?1, ?2)",
            params![identity.certificate(), identity.private_key().as_slice()],
        )?;
        Ok(())
    }

    /// Base neuve : ni compte ni coffre ; une restauration peut la remplacer.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn is_fresh(&self) -> Result<bool, StorageError> {
        let fresh = self.conn.query_row(
            "SELECT NOT EXISTS (SELECT 1 FROM accounts) AND NOT EXISTS (SELECT 1 FROM vaults)",
            [],
            |row| row.get(0),
        )?;
        Ok(fresh)
    }

    /// Copie cohérente et compacte de la base dans le fichier `path`, qui ne doit pas exister.
    ///
    /// # Errors
    ///
    /// Si le fichier existe déjà ou ne peut pas être écrit.
    pub fn backup_to(&self, path: &str) -> Result<(), StorageError> {
        self.conn.execute("VACUUM INTO ?1", params![path])?;
        Ok(())
    }

    /// Casse la base pour vérifier le traitement des erreurs SQLite.
    #[cfg(test)]
    pub(crate) fn break_for_tests(&self, table: &str) {
        let dropped = self.conn.execute_batch(&format!(
            "PRAGMA foreign_keys = OFF; DROP TABLE {table}; PRAGMA foreign_keys = ON;"
        ));
        assert!(dropped.is_ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pswd.db");
        let identity = Identity::from_parts(vec![1], Zeroizing::new(vec![2]));
        Storage::open(&path)
            .unwrap()
            .save_identity(&identity)
            .unwrap();
        let storage = Storage::open(&path).unwrap();
        assert_eq!(storage.identity().unwrap().unwrap().certificate(), [1]);
    }

    #[test]
    fn newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pswd.db");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        let error = Storage::open(&path).err().unwrap();
        assert!(matches!(error, StorageError::UnknownSchema(2)));
        assert_eq!(error.to_string(), "base de données de version inconnue (2)");
    }

    #[test]
    fn unopenable_file_is_a_database_error() {
        let dir = tempfile::tempdir().unwrap();
        let error = Storage::open(dir.path()).err().unwrap();
        assert!(matches!(error, StorageError::Database(_)));
        assert!(error.to_string().starts_with("base de données : "));
    }

    #[test]
    fn a_backup_is_a_readable_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mut storage = Storage::open(&dir.path().join("pswd.db")).unwrap();
        let identity = Identity::from_parts(vec![1], Zeroizing::new(vec![2]));
        storage.save_identity(&identity).unwrap();
        let copy = dir.path().join("copie.db");
        let copy_path = copy.to_str().unwrap();
        storage.backup_to(copy_path).unwrap();
        assert!(matches!(
            storage.backup_to(copy_path),
            Err(StorageError::Database(_))
        ));

        let mut restored = Storage::open_read_only(&copy).unwrap();
        assert_eq!(restored.identity().unwrap().unwrap().certificate(), [1]);
        assert!(matches!(
            restored.save_identity(&identity),
            Err(StorageError::Database(_))
        ));
    }

    #[test]
    fn only_a_pswd_database_opens_as_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("texte.db");
        std::fs::write(&text, "pas une base").unwrap();
        assert!(matches!(
            Storage::open_read_only(&text),
            Err(StorageError::Database(_))
        ));
        let other = dir.path().join("autre.db");
        Connection::open(&other).unwrap();
        assert!(matches!(
            Storage::open_read_only(&other),
            Err(StorageError::UnknownSchema(0))
        ));
        assert!(matches!(
            Storage::open_read_only(&dir.path().join("absente.db")),
            Err(StorageError::Database(_))
        ));
    }

    #[test]
    fn a_base_without_account_nor_vault_is_fresh() {
        let mut storage = Storage::open_in_memory().unwrap();
        assert!(storage.is_fresh().unwrap());
        storage
            .add_connection_key(b"cle", &KeyPurpose::Admin, crate::clock::now())
            .unwrap();
        assert!(storage.is_fresh().unwrap());
        storage
            .conn
            .execute(
                "INSERT INTO accounts (id, name, is_admin, created_at) VALUES (X'01', 'A', 1, 0)",
                [],
            )
            .unwrap();
        assert!(!storage.is_fresh().unwrap());
        storage.break_for_tests("vaults");
        assert!(matches!(storage.is_fresh(), Err(StorageError::Database(_))));
    }

    #[test]
    fn new_database_has_no_identity() {
        let storage = Storage::open_in_memory().unwrap();
        assert!(storage.identity().unwrap().is_none());
    }

    #[test]
    fn identity_is_saved_once() {
        let mut storage = Storage::open_in_memory().unwrap();
        let identity = Identity::from_parts(vec![1, 2], Zeroizing::new(vec![3, 4]));
        storage.save_identity(&identity).unwrap();
        let saved = storage.identity().unwrap().unwrap();
        assert_eq!(saved.certificate(), [1, 2]);
        assert_eq!(saved.private_key().as_slice(), [3, 4]);

        let other = Identity::from_parts(vec![5], Zeroizing::new(vec![6]));
        assert!(matches!(
            storage.save_identity(&other),
            Err(StorageError::Database(_))
        ));
        assert_eq!(storage.identity().unwrap().unwrap().certificate(), [1, 2]);
    }

    #[test]
    fn errors_have_readable_messages() {
        let messages = [
            (StorageError::NotFound, "coffre inconnu"),
            (StorageError::Gone, "coffre retiré"),
            (StorageError::Deleted, "coffre supprimé"),
            (StorageError::DeletedEverywhere, "coffre supprimé partout"),
            (StorageError::NotShared, "coffre privé"),
            (StorageError::UnknownAccount, "compte inconnu"),
            (StorageError::AlreadyMember, "déjà membre ou déjà invité"),
            (StorageError::SharedVault, "coffre commun"),
            (StorageError::HasMembers, "le coffre a encore des membres"),
            (StorageError::LastAdmin, "dernier administrateur"),
            (StorageError::OtherAdmin, "compte d'un autre administrateur"),
            (
                StorageError::Conflict,
                "le coffre a changé depuis la version lue",
            ),
            (
                StorageError::InvalidKey,
                "clé de connexion invalide ou expirée",
            ),
            (StorageError::MissingAccountName, "nom du compte manquant"),
            (StorageError::NameTaken, "nom de compte déjà pris"),
            (
                StorageError::RandomUnavailable,
                "générateur aléatoire du système indisponible",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
