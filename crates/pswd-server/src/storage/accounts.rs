//! Comptes, appareils et clés de connexion. Les secrets (jetons, clés) ne sont jamais rangés :
//! seulement leur empreinte.

use pswd_core::Timestamp;
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

use super::{Storage, StorageError};

/// Appareil authentifié par son jeton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caller {
    pub account: Uuid,
    pub device: Uuid,
    pub is_admin: bool,
}

/// Ce que donne une clé de connexion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyPurpose {
    /// Un nouveau compte administrateur, nommé par l'appareil qui présente la clé.
    Admin,
    /// Un nouveau compte membre, nommé par l'administrateur.
    Member { name: String },
    /// Un appareil de plus pour un compte existant.
    Device { account: Uuid },
}

/// Appareil qui présente une clé de connexion.
pub struct NewDevice<'a> {
    pub name: &'a str,
    pub token_hash: &'a [u8],
    /// Nom du compte à créer, pour une clé administrateur.
    pub account_name: Option<&'a str>,
}

impl Storage {
    /// Enregistre l'empreinte d'une nouvelle clé de connexion. Une nouvelle clé administrateur
    /// annule les précédentes. Les clés expirées sont supprimées au passage.
    ///
    /// # Errors
    ///
    /// Si l'écriture dans la base échoue.
    pub fn add_connection_key(
        &mut self,
        secret_hash: &[u8],
        purpose: &KeyPurpose,
        expires_at: Timestamp,
    ) -> Result<(), StorageError> {
        let (account_id, account_name) = match purpose {
            KeyPurpose::Admin => (None, None),
            KeyPurpose::Member { name } => (None, Some(name.as_str())),
            KeyPurpose::Device { account } => (Some(*account), None),
        };
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM connection_keys WHERE expires_at <= ?1",
            params![now_millis()],
        )?;
        if *purpose == KeyPurpose::Admin {
            tx.execute(
                "DELETE FROM connection_keys WHERE account_id IS NULL AND account_name IS NULL",
                [],
            )?;
        }
        tx.execute(
            "INSERT INTO connection_keys (secret_hash, account_id, account_name, expires_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                secret_hash,
                account_id,
                account_name,
                expires_at.as_millis()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Consomme une clé de connexion encore valide et enregistre l'appareil qui la présente.
    ///
    /// # Errors
    ///
    /// [`StorageError::InvalidKey`] si la clé est inconnue, expirée ou déjà utilisée ;
    /// [`StorageError::MissingAccountName`] pour une clé administrateur sans nom de compte. La
    /// clé n'est alors pas consommée.
    pub fn redeem_connection_key(
        &mut self,
        secret_hash: &[u8],
        now: Timestamp,
        device: &NewDevice<'_>,
    ) -> Result<Caller, StorageError> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (account_id, account_name): (Option<Uuid>, Option<String>) = tx
            .query_row(
                "SELECT account_id, account_name FROM connection_keys
                 WHERE secret_hash = ?1 AND expires_at > ?2",
                params![secret_hash, now.as_millis()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(StorageError::InvalidKey)?;
        tx.execute(
            "DELETE FROM connection_keys WHERE secret_hash = ?1",
            params![secret_hash],
        )?;
        let (account, is_admin) = match (account_id, account_name) {
            (Some(account), _) => {
                let is_admin = tx.query_row(
                    "SELECT is_admin FROM accounts WHERE id = ?1",
                    params![account],
                    |row| row.get(0),
                )?;
                (account, is_admin)
            }
            (None, name) => {
                let is_admin = name.is_none();
                let name = name
                    .as_deref()
                    .or(device.account_name)
                    .ok_or(StorageError::MissingAccountName)?;
                let account = Uuid::new_v4();
                tx.execute(
                    "INSERT INTO accounts (id, name, is_admin, created_at) VALUES (?1, ?2, ?3, ?4)",
                    params![account, name, is_admin, now.as_millis()],
                )?;
                (account, is_admin)
            }
        };
        let id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO devices (id, account_id, name, token_hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, account, device.name, device.token_hash, now.as_millis()],
        )?;
        tx.commit()?;
        Ok(Caller {
            account,
            device: id,
            is_admin,
        })
    }

    /// Appareil dont le jeton a l'empreinte `token_hash` ; `None` s'il est inconnu ou révoqué.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn authenticate(&self, token_hash: &[u8]) -> Result<Option<Caller>, StorageError> {
        let caller = self
            .conn
            .query_row(
                "SELECT d.id, a.id, a.is_admin FROM devices d
                 JOIN accounts a ON a.id = d.account_id
                 WHERE d.token_hash = ?1",
                params![token_hash],
                |row| {
                    Ok(Caller {
                        device: row.get(0)?,
                        account: row.get(1)?,
                        is_admin: row.get(2)?,
                    })
                },
            )
            .optional()?;
        Ok(caller)
    }

    /// Révoque un appareil : son jeton ne vaut plus rien.
    ///
    /// # Errors
    ///
    /// Si l'écriture dans la base échoue.
    pub fn revoke_device(&mut self, device: Uuid) -> Result<(), StorageError> {
        self.conn
            .execute("DELETE FROM devices WHERE id = ?1", params![device])?;
        Ok(())
    }
}

fn now_millis() -> i64 {
    crate::clock::now().as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);
    const LATER: Timestamp = Timestamp::from_millis(i64::MAX);

    fn storage() -> Storage {
        Storage::open_in_memory().unwrap()
    }

    fn device<'a>(token_hash: &'a [u8], account_name: Option<&'a str>) -> NewDevice<'a> {
        NewDevice {
            name: "PC",
            token_hash,
            account_name,
        }
    }

    fn key_count(storage: &Storage) -> i64 {
        storage
            .conn
            .query_row("SELECT COUNT(*) FROM connection_keys", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn admin_key_creates_an_admin_account() {
        let mut storage = storage();
        storage
            .add_connection_key(b"cle", &KeyPurpose::Admin, LATER)
            .unwrap();
        let caller = storage
            .redeem_connection_key(b"cle", NOW, &device(b"jeton", Some("Arthur")))
            .unwrap();
        assert!(caller.is_admin);
        assert_eq!(storage.authenticate(b"jeton").unwrap(), Some(caller));
        let name: String = storage
            .conn
            .query_row(
                "SELECT name FROM accounts WHERE id = ?1",
                params![caller.account],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "Arthur");
    }

    #[test]
    fn admin_key_needs_an_account_name() {
        let mut storage = storage();
        storage
            .add_connection_key(b"cle", &KeyPurpose::Admin, LATER)
            .unwrap();
        assert!(matches!(
            storage.redeem_connection_key(b"cle", NOW, &device(b"jeton", None)),
            Err(StorageError::MissingAccountName)
        ));
        assert!(
            storage
                .redeem_connection_key(b"cle", NOW, &device(b"jeton", Some("Arthur")))
                .is_ok()
        );
    }

    #[test]
    fn member_key_creates_a_named_member_account() {
        let mut storage = storage();
        let purpose = KeyPurpose::Member {
            name: "Camille".to_owned(),
        };
        storage.add_connection_key(b"cle", &purpose, LATER).unwrap();
        let caller = storage
            .redeem_connection_key(b"cle", NOW, &device(b"jeton", Some("ignoré")))
            .unwrap();
        assert!(!caller.is_admin);
        let name: String = storage
            .conn
            .query_row(
                "SELECT name FROM accounts WHERE id = ?1",
                params![caller.account],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(name, "Camille");
    }

    #[test]
    fn device_key_adds_a_device_to_the_account() {
        let mut storage = storage();
        storage
            .add_connection_key(b"admin", &KeyPurpose::Admin, LATER)
            .unwrap();
        let first = storage
            .redeem_connection_key(b"admin", NOW, &device(b"jeton1", Some("Arthur")))
            .unwrap();
        let purpose = KeyPurpose::Device {
            account: first.account,
        };
        storage.add_connection_key(b"cle", &purpose, LATER).unwrap();
        let second = storage
            .redeem_connection_key(b"cle", NOW, &device(b"jeton2", None))
            .unwrap();
        assert_eq!(second.account, first.account);
        assert!(second.is_admin);
        assert_ne!(second.device, first.device);
    }

    #[test]
    fn device_key_for_a_vanished_account_fails() {
        let mut storage = storage();
        let purpose = KeyPurpose::Device {
            account: Uuid::new_v4(),
        };
        storage.break_for_tests("connection_keys");
        storage
            .conn
            .execute_batch(
                "CREATE TABLE connection_keys (secret_hash BLOB PRIMARY KEY, account_id BLOB,
                 account_name TEXT, expires_at INTEGER NOT NULL)",
            )
            .unwrap();
        storage.add_connection_key(b"cle", &purpose, LATER).unwrap();
        assert!(matches!(
            storage.redeem_connection_key(b"cle", NOW, &device(b"jeton", None)),
            Err(StorageError::Database(_))
        ));
    }

    #[test]
    fn key_is_used_only_once() {
        let mut storage = storage();
        storage
            .add_connection_key(b"cle", &KeyPurpose::Admin, LATER)
            .unwrap();
        storage
            .redeem_connection_key(b"cle", NOW, &device(b"jeton1", Some("A")))
            .unwrap();
        assert!(matches!(
            storage.redeem_connection_key(b"cle", NOW, &device(b"jeton2", Some("A"))),
            Err(StorageError::InvalidKey)
        ));
    }

    #[test]
    fn unknown_or_expired_key_is_invalid() {
        let mut storage = storage();
        storage
            .add_connection_key(b"cle", &KeyPurpose::Admin, NOW)
            .unwrap();
        for secret in [&b"cle"[..], b"autre"] {
            assert!(matches!(
                storage.redeem_connection_key(secret, NOW, &device(b"jeton", Some("A"))),
                Err(StorageError::InvalidKey)
            ));
        }
        let before = Timestamp::from_millis(NOW.as_millis() - 1);
        assert!(
            storage
                .redeem_connection_key(b"cle", before, &device(b"jeton", Some("A")))
                .is_ok()
        );
    }

    #[test]
    fn new_admin_key_cancels_the_previous_ones() {
        let mut storage = storage();
        storage
            .add_connection_key(b"ancienne", &KeyPurpose::Admin, LATER)
            .unwrap();
        let member = KeyPurpose::Member {
            name: "Camille".to_owned(),
        };
        storage
            .add_connection_key(b"membre", &member, LATER)
            .unwrap();
        storage
            .add_connection_key(b"nouvelle", &KeyPurpose::Admin, LATER)
            .unwrap();
        assert!(matches!(
            storage.redeem_connection_key(b"ancienne", NOW, &device(b"j1", Some("A"))),
            Err(StorageError::InvalidKey)
        ));
        assert!(
            storage
                .redeem_connection_key(b"membre", NOW, &device(b"j2", None))
                .is_ok()
        );
        assert!(
            storage
                .redeem_connection_key(b"nouvelle", NOW, &device(b"j3", Some("A")))
                .is_ok()
        );
    }

    #[test]
    fn expired_keys_are_removed() {
        let mut storage = storage();
        let past = Timestamp::from_millis(1);
        let member = KeyPurpose::Member {
            name: "Camille".to_owned(),
        };
        storage
            .add_connection_key(b"vieille", &member, past)
            .unwrap();
        storage
            .add_connection_key(b"neuve", &member, LATER)
            .unwrap();
        assert_eq!(key_count(&storage), 1);
    }

    #[test]
    fn duplicate_token_is_refused_and_keeps_the_key() {
        let mut storage = storage();
        storage
            .add_connection_key(b"cle1", &KeyPurpose::Admin, LATER)
            .unwrap();
        storage
            .redeem_connection_key(b"cle1", NOW, &device(b"jeton", Some("A")))
            .unwrap();
        let member = KeyPurpose::Member {
            name: "Camille".to_owned(),
        };
        storage.add_connection_key(b"cle2", &member, LATER).unwrap();
        assert!(matches!(
            storage.redeem_connection_key(b"cle2", NOW, &device(b"jeton", None)),
            Err(StorageError::Database(_))
        ));
        assert_eq!(key_count(&storage), 1);
    }

    #[test]
    fn revoked_device_is_no_longer_authenticated() {
        let mut storage = storage();
        storage
            .add_connection_key(b"cle", &KeyPurpose::Admin, LATER)
            .unwrap();
        let caller = storage
            .redeem_connection_key(b"cle", NOW, &device(b"jeton", Some("A")))
            .unwrap();
        storage.revoke_device(caller.device).unwrap();
        assert_eq!(storage.authenticate(b"jeton").unwrap(), None);
        assert_eq!(storage.authenticate(b"inconnu").unwrap(), None);
    }

    #[test]
    fn database_failures_are_reported() {
        let mut storage = storage();
        storage.break_for_tests("devices");
        assert!(matches!(
            storage.authenticate(b"jeton"),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            storage.revoke_device(Uuid::new_v4()),
            Err(StorageError::Database(_))
        ));
        storage.break_for_tests("connection_keys");
        assert!(matches!(
            storage.add_connection_key(b"cle", &KeyPurpose::Admin, LATER),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            storage.redeem_connection_key(b"cle", NOW, &device(b"jeton", Some("A"))),
            Err(StorageError::Database(_))
        ));
    }
}
