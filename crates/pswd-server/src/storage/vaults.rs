//! Coffres, leurs membres et leurs dernières versions.

use pswd_core::Timestamp;
use rusqlite::TransactionBehavior::Immediate;
use rusqlite::{Connection, OptionalExtension, Row, params};
use uuid::Uuid;

use super::{Storage, StorageError};

/// Nombre de versions conservées par coffre, la version courante comprise.
pub const KEPT_VERSIONS: i64 = 15;

/// Valeurs de `vaults.removed`, après 0 (coffre présent).
pub(super) const REMOVED_BY_ADMIN: i64 = 1;
pub(super) const REMOVED_EVERYWHERE: i64 = 2;

const LIST_VAULTS: &str = "SELECT v.id, v.name, v.shared, v.current_version FROM vaults v
     JOIN vault_members m ON m.vault_id = v.id
     WHERE m.account_id = ?1 ORDER BY v.id";
const INSERT_VERSION: &str = "INSERT INTO vault_versions (vault_id, version, content, created_at)
     VALUES (?1, ?2, ?3, ?4)";
const SET_CURRENT_VERSION: &str = "UPDATE vaults SET current_version = ?2 WHERE id = ?1";
const DELETE_OLD_VERSIONS: &str = "DELETE FROM vault_versions WHERE vault_id = ?1 AND version < ?2";

/// Contenu courant d'un coffre.
#[derive(Debug, PartialEq, Eq)]
pub struct StoredFile {
    pub content: Vec<u8>,
    pub version: i64,
}

/// Coffre dont un compte est membre ; `version` vaut `None` tant qu'il n'est pas publié.
#[derive(Debug, PartialEq, Eq)]
pub struct VaultSummary {
    pub id: Uuid,
    pub name: String,
    /// Coffre commun.
    pub shared: bool,
    pub version: Option<i64>,
}

impl Storage {
    /// Réserve l'identifiant d'un nouveau coffre `name`, encore vide, dont `account` est membre ;
    /// `shared` en fait un coffre commun.
    ///
    /// # Errors
    ///
    /// Si l'écriture dans la base échoue.
    pub fn create_vault(
        &mut self,
        account: Uuid,
        name: &str,
        shared: bool,
    ) -> Result<Uuid, StorageError> {
        let id = Uuid::new_v4();
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO vaults (id, name, shared, current_version) VALUES (?1, ?2, ?3, NULL)",
            params![id, name, shared],
        )?;
        tx.execute(
            "INSERT INTO vault_members (vault_id, account_id) VALUES (?1, ?2)",
            params![id, account],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Coffres dont `account` est membre, triés par identifiant.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn list_vaults(&self, account: Uuid) -> Result<Vec<VaultSummary>, StorageError> {
        let mut statement = self.conn.prepare(LIST_VAULTS)?;
        let rows = statement.query_map(params![account], summary)?;
        let vaults = rows.collect::<Result<_, _>>()?;
        Ok(vaults)
    }

    /// Contenu courant du coffre ; `None` s'il n'a encore jamais été publié.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si le coffre n'existe pas ou si `account` n'en est pas membre,
    /// [`StorageError::Gone`] s'il l'a quitté ou en a été retiré, [`StorageError::Deleted`] si le
    /// coffre a été supprimé.
    pub fn fetch(&self, id: Uuid, account: Uuid) -> Result<Option<StoredFile>, StorageError> {
        let Some(version) = current_version(&self.conn, id, account)? else {
            return Ok(None);
        };
        let content = self.conn.query_row(
            "SELECT content FROM vault_versions WHERE vault_id = ?1 AND version = ?2",
            params![id, version],
            |row| row.get(0),
        )?;
        Ok(Some(StoredFile { content, version }))
    }

    /// Publie `content` si le coffre est encore à la version `expected` (`None` : jamais publié),
    /// et renvoie la nouvelle version. Les versions au-delà des [`KEPT_VERSIONS`] dernières sont
    /// supprimées dans la même transaction.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si le coffre n'existe pas ou si `account` n'en est pas membre,
    /// [`StorageError::Gone`] s'il l'a quitté ou en a été retiré, [`StorageError::Deleted`] si le
    /// coffre a été supprimé, [`StorageError::Conflict`] s'il a changé depuis `expected`.
    pub fn publish(
        &mut self,
        id: Uuid,
        account: Uuid,
        content: &[u8],
        expected: Option<i64>,
        now: Timestamp,
    ) -> Result<i64, StorageError> {
        let tx = self.conn.transaction_with_behavior(Immediate)?;
        let current = current_version(&tx, id, account)?;
        if current != expected {
            return Err(StorageError::Conflict);
        }
        let version = current.unwrap_or(0) + 1;
        let oldest_kept = version - KEPT_VERSIONS + 1;
        tx.execute(
            INSERT_VERSION,
            params![id, version, content, now.as_millis()],
        )?;
        tx.execute(SET_CURRENT_VERSION, params![id, version])?;
        tx.execute(DELETE_OLD_VERSIONS, params![id, oldest_kept])?;
        tx.commit()?;
        Ok(version)
    }
}

pub(super) fn summary(row: &Row<'_>) -> rusqlite::Result<VaultSummary> {
    Ok(VaultSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        shared: row.get(2)?,
        version: row.get(3)?,
    })
}

/// Version courante du coffre, vu par `account`.
fn current_version(
    conn: &Connection,
    id: Uuid,
    account: Uuid,
) -> Result<Option<i64>, StorageError> {
    let version = conn
        .query_row(
            "SELECT v.current_version FROM vaults v
             JOIN vault_members m ON m.vault_id = v.id
             WHERE v.id = ?1 AND m.account_id = ?2",
            params![id, account],
            |row| row.get(0),
        )
        .optional()?;
    version.ok_or_else(|| absence(conn, id, account))
}

/// Vérifie que `account` est membre du coffre, et renvoie s'il est commun.
pub(super) fn shared_if_member(
    conn: &Connection,
    id: Uuid,
    account: Uuid,
) -> Result<bool, StorageError> {
    let shared = conn
        .query_row(
            "SELECT v.shared FROM vaults v
             JOIN vault_members m ON m.vault_id = v.id
             WHERE v.id = ?1 AND m.account_id = ?2",
            params![id, account],
            |row| row.get(0),
        )
        .optional()?;
    shared.ok_or_else(|| absence(conn, id, account))
}

/// Pourquoi `account` n'accède pas au coffre : le coffre a été supprimé
/// ([`StorageError::Deleted`], [`StorageError::DeletedEverywhere`]), il l'a quitté ou en a été
/// retiré ([`StorageError::Gone`]), ou ne l'a jamais eu ([`StorageError::NotFound`]).
fn absence(conn: &Connection, id: Uuid, account: Uuid) -> StorageError {
    let deleted = conn
        .query_row(
            "SELECT v.removed FROM former_members f JOIN vaults v ON v.id = f.vault_id
             WHERE f.vault_id = ?1 AND f.account_id = ?2",
            params![id, account],
            |row| row.get(0),
        )
        .optional();
    match deleted {
        Ok(Some(REMOVED_BY_ADMIN)) => StorageError::Deleted,
        Ok(Some(REMOVED_EVERYWHERE)) => StorageError::DeletedEverywhere,
        Ok(Some(_)) => StorageError::Gone,
        Ok(None) => StorageError::NotFound,
        Err(error) => error.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{KeyPurpose, NewDevice};

    const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

    /// Base avec deux comptes membres.
    fn storage() -> (Storage, Uuid, Uuid) {
        let mut storage = Storage::open_in_memory().unwrap();
        let first = member(&mut storage, b"premier");
        let second = member(&mut storage, b"second");
        (storage, first, second)
    }

    fn member(storage: &mut Storage, secret: &[u8]) -> Uuid {
        let purpose = KeyPurpose::Member {
            name: String::from_utf8_lossy(secret).into_owned(),
        };
        storage
            .add_connection_key(secret, &purpose, Timestamp::from_millis(i64::MAX))
            .unwrap();
        let device = NewDevice {
            name: "PC",
            token_hash: secret,
            account_name: None,
        };
        storage
            .redeem_connection_key(secret, NOW, &device)
            .unwrap()
            .account
    }

    fn version_count(storage: &Storage, id: Uuid) -> i64 {
        storage
            .conn
            .query_row(
                "SELECT COUNT(*) FROM vault_versions WHERE vault_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap()
    }

    #[test]
    fn new_vault_is_empty() {
        let (mut storage, account, _) = storage();
        let id = storage.create_vault(account, "perso", false).unwrap();
        assert_eq!(storage.fetch(id, account).unwrap(), None);
    }

    #[test]
    fn unknown_vault_is_not_found() {
        let (storage, account, _) = storage();
        assert!(matches!(
            storage.fetch(Uuid::new_v4(), account),
            Err(StorageError::NotFound)
        ));
    }

    #[test]
    fn vault_of_another_account_is_not_found() {
        let (mut storage, owner, other) = storage();
        let id = storage.create_vault(owner, "perso", false).unwrap();
        storage.publish(id, owner, b"a", None, NOW).unwrap();
        assert!(matches!(
            storage.fetch(id, other),
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            storage.publish(id, other, b"b", Some(1), NOW),
            Err(StorageError::NotFound)
        ));
        assert_eq!(storage.fetch(id, owner).unwrap().unwrap().content, b"a");
    }

    #[test]
    fn creation_fails_for_an_unknown_account() {
        let (mut storage, _, _) = storage();
        assert!(matches!(
            storage.create_vault(Uuid::new_v4(), "perso", false),
            Err(StorageError::Database(_))
        ));
        assert_eq!(
            storage
                .conn
                .query_row("SELECT COUNT(*) FROM vaults", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn creation_fails_on_a_broken_database() {
        let (mut storage, account, _) = storage();
        storage.break_for_tests("vaults");
        assert!(matches!(
            storage.create_vault(account, "perso", false),
            Err(StorageError::Database(_))
        ));
    }

    #[test]
    fn accounts_list_their_own_vaults() {
        let (mut storage, first, second) = storage();
        let a = storage.create_vault(first, "perso", false).unwrap();
        let b = storage.create_vault(first, "travail", true).unwrap();
        let c = storage.create_vault(second, "famille", false).unwrap();
        storage.publish(b, first, b"x", None, NOW).unwrap();

        let mut expected = vec![
            VaultSummary {
                id: a,
                name: "perso".to_owned(),
                shared: false,
                version: None,
            },
            VaultSummary {
                id: b,
                name: "travail".to_owned(),
                shared: true,
                version: Some(1),
            },
        ];
        expected.sort_by_key(|vault| vault.id);
        assert_eq!(storage.list_vaults(first).unwrap(), expected);
        assert_eq!(
            storage.list_vaults(second).unwrap(),
            [VaultSummary {
                id: c,
                name: "famille".to_owned(),
                shared: false,
                version: None
            }]
        );
    }

    #[test]
    fn first_publication_requires_an_empty_vault() {
        let (mut storage, account, _) = storage();
        let id = storage.create_vault(account, "perso", false).unwrap();
        assert!(matches!(
            storage.publish(id, account, b"a", Some(1), NOW),
            Err(StorageError::Conflict)
        ));
        assert_eq!(storage.publish(id, account, b"a", None, NOW).unwrap(), 1);
        assert_eq!(
            storage.fetch(id, account).unwrap(),
            Some(StoredFile {
                content: b"a".to_vec(),
                version: 1
            })
        );
    }

    #[test]
    fn publication_requires_the_current_version() {
        let (mut storage, account, _) = storage();
        let id = storage.create_vault(account, "perso", false).unwrap();
        storage.publish(id, account, b"a", None, NOW).unwrap();
        for expected in [None, Some(2)] {
            assert!(matches!(
                storage.publish(id, account, b"b", expected, NOW),
                Err(StorageError::Conflict)
            ));
        }
        assert_eq!(storage.publish(id, account, b"b", Some(1), NOW).unwrap(), 2);
        assert_eq!(storage.fetch(id, account).unwrap().unwrap().content, b"b");
    }

    #[test]
    fn only_the_last_versions_are_kept() {
        let (mut storage, account, _) = storage();
        let id = storage.create_vault(account, "perso", false).unwrap();
        let mut expected = None;
        for version in 1..=KEPT_VERSIONS + 2 {
            expected = Some(storage.publish(id, account, b"x", expected, NOW).unwrap());
            assert_eq!(version_count(&storage, id), version.min(KEPT_VERSIONS));
        }
        let oldest: i64 = storage
            .conn
            .query_row(
                "SELECT MIN(version) FROM vault_versions WHERE vault_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(oldest, 3);
    }

    #[test]
    fn failed_publication_changes_nothing() {
        let (mut storage, account, _) = storage();
        let id = storage.create_vault(account, "perso", false).unwrap();
        storage.break_for_tests("vault_versions");
        assert!(matches!(
            storage.publish(id, account, b"a", None, NOW),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(storage.fetch(id, account), Ok(None)));
    }

    #[test]
    fn unreadable_content_is_a_database_error() {
        let (mut storage, account, _) = storage();
        let id = storage.create_vault(account, "perso", false).unwrap();
        storage.publish(id, account, b"a", None, NOW).unwrap();
        storage.break_for_tests("vault_versions");
        assert!(matches!(
            storage.fetch(id, account),
            Err(StorageError::Database(_))
        ));
    }

    #[test]
    fn unreadable_vault_list_is_a_database_error() {
        let (storage, account, _) = storage();
        storage.break_for_tests("vault_members");
        assert!(matches!(
            storage.fetch(Uuid::new_v4(), account),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            storage.list_vaults(account),
            Err(StorageError::Database(_))
        ));
    }

    #[test]
    fn unreadable_vault_row_is_a_database_error() {
        let (mut storage, account, _) = storage();
        let id = storage.create_vault(account, "perso", false).unwrap();
        storage
            .conn
            .execute(
                "UPDATE vaults SET current_version = 'abc' WHERE id = ?1",
                params![id],
            )
            .unwrap();
        assert!(matches!(
            storage.list_vaults(account),
            Err(StorageError::Database(_))
        ));
    }
}
