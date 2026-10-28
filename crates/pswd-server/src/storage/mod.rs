//! Base SQLite du serveur : coffres opaques et leurs versions, comptes, appareils, certificat.

mod accounts;
mod sharing;
mod vaults;

use std::fmt;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use zeroize::Zeroizing;

use crate::tls::Identity;

pub use accounts::{Caller, KeyPurpose, NewDevice};
pub use sharing::{AccountSummary, Members, ReceivedInvitation, SentInvitation};
pub use vaults::{KEPT_VERSIONS, StoredFile, VaultSummary};

/// Version actuelle du schéma, rangée dans `PRAGMA user_version`.
const SCHEMA_VERSION: i64 = 1;

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
        account_id BLOB REFERENCES accounts(id),
        account_name TEXT,
        expires_at INTEGER NOT NULL
    );
    CREATE TABLE vaults (
        id BLOB PRIMARY KEY,
        name TEXT NOT NULL,
        shared INTEGER NOT NULL,
        current_version INTEGER
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
";

#[derive(Debug)]
pub enum StorageError {
    /// Coffre ou invitation inconnus, ou dont le compte n'est pas membre.
    NotFound,
    /// Le compte a quitté le coffre ou en a été retiré.
    Gone,
    /// Un coffre privé ne se partage pas et ne se quitte pas.
    NotShared,
    /// Le compte invité n'existe pas.
    UnknownAccount,
    /// Le compte invité est déjà membre ou déjà invité.
    AlreadyMember,
    /// Le coffre a changé depuis la version attendue.
    Conflict,
    /// Clé de connexion inconnue, expirée ou déjà utilisée.
    InvalidKey,
    /// Une clé administrateur crée un compte : il lui faut un nom.
    MissingAccountName,
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
            Self::NotShared => f.write_str("coffre privé"),
            Self::UnknownAccount => f.write_str("compte inconnu"),
            Self::AlreadyMember => f.write_str("déjà membre ou déjà invité"),
            Self::Conflict => f.write_str("le coffre a changé depuis la version lue"),
            Self::InvalidKey => f.write_str("clé de connexion invalide ou expirée"),
            Self::MissingAccountName => f.write_str("nom du compte manquant"),
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

    fn init(mut conn: Connection) -> Result<Self, StorageError> {
        conn.pragma_update(None, "foreign_keys", true)?;
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
            (StorageError::NotShared, "coffre privé"),
            (StorageError::UnknownAccount, "compte inconnu"),
            (StorageError::AlreadyMember, "déjà membre ou déjà invité"),
            (
                StorageError::Conflict,
                "le coffre a changé depuis la version lue",
            ),
            (
                StorageError::InvalidKey,
                "clé de connexion invalide ou expirée",
            ),
            (StorageError::MissingAccountName, "nom du compte manquant"),
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
