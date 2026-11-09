//! Administration : comptes et appareils, clés en attente, coffres de tous les comptes, versions,
//! adresse publique et sauvegarde de la base.

use pswd_core::{MAX_VAULT_NAME_LEN, Timestamp};
use rusqlite::TransactionBehavior::Immediate;
use rusqlite::{Connection, MAIN_DB, OptionalExtension, Row, params};
use uuid::Uuid;

use super::sharing::{
    ACCOUNT_EXISTS, AccountSummary, DELETE_FORMER, DELETE_MEMBER, INSERT_FORMER, INSERT_MEMBER,
    LIST_MEMBERS, account, exists,
};
use super::vaults::{REMOVED_BY_ADMIN, REMOVED_EVERYWHERE, summary};
use super::{Storage, StorageError};

const LIST_ACCOUNTS: &str = "SELECT id, name, is_admin FROM accounts ORDER BY name, id";
const LIST_DEVICES: &str = "SELECT id, name, created_at FROM devices
     WHERE account_id = ?1 ORDER BY created_at, id";
/// Rôle du compte, et nombre d'administrateurs du serveur.
const ADMIN_STATUS: &str = "SELECT is_admin, (SELECT COUNT(*) FROM accounts WHERE is_admin)
     FROM accounts WHERE id = ?1";
/// Tout ce qui renvoie au compte, puis le compte lui-même.
const DELETE_ACCOUNT: [&str; 6] = [
    "DELETE FROM devices WHERE account_id = ?1",
    "DELETE FROM connection_keys WHERE account_id = ?1",
    "DELETE FROM invitations WHERE account_id = ?1 OR invited_by = ?1",
    "DELETE FROM vault_members WHERE account_id = ?1",
    "DELETE FROM former_members WHERE account_id = ?1",
    "DELETE FROM accounts WHERE id = ?1",
];
const LIST_KEYS: &str = "SELECT k.id, k.account_name, a.name, k.expires_at
     FROM connection_keys k LEFT JOIN accounts a ON a.id = k.account_id
     WHERE k.expires_at > ?1 ORDER BY k.expires_at, k.id";
const LIST_SERVER_VAULTS: &str = "SELECT id, name, shared, current_version FROM vaults
     WHERE NOT removed ORDER BY name, id";
const VAULT_EXISTS: &str = "SELECT EXISTS (SELECT 1 FROM vaults WHERE id = ?1 AND NOT removed)";
const VAULT_SHARED: &str = "SELECT shared FROM vaults WHERE id = ?1 AND NOT removed";
const HAS_MEMBERS: &str = "SELECT EXISTS (SELECT 1 FROM vault_members WHERE vault_id = ?1)";
const OTHER_MEMBERS: &str =
    "SELECT EXISTS (SELECT 1 FROM vault_members WHERE vault_id = ?1 AND account_id != ?2)";
/// Les membres deviennent d'anciens membres ; le contenu, les versions et les invitations sont
/// effacés.
const REMOVE_VAULT: [&str; 4] = [
    "INSERT OR IGNORE INTO former_members (vault_id, account_id)
     SELECT vault_id, account_id FROM vault_members WHERE vault_id = ?1",
    "DELETE FROM vault_members WHERE vault_id = ?1",
    "DELETE FROM invitations WHERE vault_id = ?1",
    "DELETE FROM vault_versions WHERE vault_id = ?1",
];
const MARK_REMOVED: &str = "UPDATE vaults SET removed = ?2, current_version = NULL WHERE id = ?1";
const DELETE_INVITATION_TO: &str =
    "DELETE FROM invitations WHERE vault_id = ?1 AND account_id = ?2";
const INSERT_RESTORED: &str =
    "INSERT INTO vaults (id, name, shared, current_version) VALUES (?1, ?2, ?3, 1)";
const LIST_VERSIONS: &str = "SELECT version, created_at, length(content) FROM vault_versions
     WHERE vault_id = ?1 ORDER BY version DESC";
const COPY_MEMBERS: &str = "INSERT INTO vault_members (vault_id, account_id)
     SELECT ?1, account_id FROM vault_members WHERE vault_id = ?2";
const COPY_VERSION: &str = "INSERT INTO vault_versions (vault_id, version, content, created_at)
     SELECT ?1, 1, content, ?2 FROM vault_versions WHERE vault_id = ?3 AND version = ?4";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceSummary {
    pub id: Uuid,
    pub name: String,
    pub created_at: Timestamp,
}

/// Compte du serveur et ses appareils, du plus ancien au plus récent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountDetails {
    pub id: Uuid,
    pub name: String,
    pub is_admin: bool,
    pub devices: Vec<DeviceSummary>,
}

/// Ce que donnera une clé de connexion en attente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingPurpose {
    Admin,
    /// Un nouveau compte membre, de ce nom.
    Member {
        name: String,
    },
    /// Un appareil de plus pour le compte de ce nom.
    Device {
        account: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingKey {
    pub id: Uuid,
    pub purpose: PendingPurpose,
    pub expires_at: Timestamp,
}

/// Coffre vu par un administrateur ; `version` vaut `None` tant qu'il n'est pas publié.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerVault {
    pub id: Uuid,
    pub name: String,
    pub shared: bool,
    pub version: Option<i64>,
    pub members: Vec<AccountSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSummary {
    pub version: i64,
    pub created_at: Timestamp,
    /// Taille du fichier, en octets.
    pub size: i64,
}

/// Coffre créé par la restauration d'une version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    pub id: Uuid,
    pub name: String,
}

impl Storage {
    /// Comptes du serveur, triés par nom, avec leurs appareils.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn account_details(&self) -> Result<Vec<AccountDetails>, StorageError> {
        let mut accounts = self.conn.prepare(LIST_ACCOUNTS)?;
        let mut devices = self.conn.prepare(LIST_DEVICES)?;
        let rows = accounts.query_map([], account_row)?;
        let mut details = Vec::new();
        for row in rows {
            let (id, name, is_admin) = row?;
            let devices = devices.query_map(params![id], device)?;
            details.push(AccountDetails {
                id,
                name,
                is_admin,
                devices: devices.collect::<Result<_, _>>()?,
            });
        }
        Ok(details)
    }

    /// Supprime le compte, ses appareils, ses clés et ses invitations. Ses coffres restent sur le
    /// serveur ; ceux dont il était le seul membre n'en ont plus.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si le compte n'existe pas, [`StorageError::LastAdmin`] pour le
    /// dernier administrateur.
    pub fn delete_account(&mut self, account: Uuid) -> Result<(), StorageError> {
        let tx = self.conn.transaction_with_behavior(Immediate)?;
        ensure_not_last_admin(&tx, account)?;
        for statement in DELETE_ACCOUNT {
            tx.execute(statement, params![account])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Donne ou retire le rôle d'administrateur.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si le compte n'existe pas, [`StorageError::LastAdmin`] pour
    /// retirer son rôle au dernier administrateur.
    pub fn set_admin(&mut self, account: Uuid, is_admin: bool) -> Result<(), StorageError> {
        let tx = self.conn.transaction_with_behavior(Immediate)?;
        if !is_admin {
            ensure_not_last_admin(&tx, account)?;
        }
        let updated = tx.execute(
            "UPDATE accounts SET is_admin = ?2 WHERE id = ?1",
            params![account, is_admin],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound);
        }
        tx.commit()?;
        Ok(())
    }

    /// Clés de connexion encore valables, de la plus proche de son expiration à la plus lointaine.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn pending_keys(&self, now: Timestamp) -> Result<Vec<PendingKey>, StorageError> {
        let mut statement = self.conn.prepare(LIST_KEYS)?;
        let rows = statement.query_map(params![now.as_millis()], pending_key)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Annule une clé de connexion encore inutilisée.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si la clé n'existe pas ou a déjà servi.
    pub fn cancel_key(&mut self, key: Uuid) -> Result<(), StorageError> {
        let deleted = self
            .conn
            .execute("DELETE FROM connection_keys WHERE id = ?1", params![key])?;
        if deleted == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// Coffres du serveur, de tous les comptes, triés par nom ; les coffres supprimés n'y sont pas.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn server_vaults(&self) -> Result<Vec<ServerVault>, StorageError> {
        let mut vaults = self.conn.prepare(LIST_SERVER_VAULTS)?;
        let mut members = self.conn.prepare(LIST_MEMBERS)?;
        let rows = vaults.query_map([], summary)?;
        let mut listed = Vec::new();
        for row in rows {
            let vault = row?;
            let members = members.query_map(params![vault.id], account)?;
            listed.push(ServerVault {
                id: vault.id,
                name: vault.name,
                shared: vault.shared,
                version: vault.version,
                members: members.collect::<Result<_, _>>()?,
            });
        }
        Ok(listed)
    }

    /// Supprime le coffre : son contenu et ses versions sont effacés, ses membres le voient retiré.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si le coffre n'existe pas ou est déjà supprimé.
    pub fn delete_vault(&mut self, vault: Uuid) -> Result<(), StorageError> {
        let tx = self.conn.transaction_with_behavior(Immediate)?;
        ensure_vault_exists(&tx, vault)?;
        remove_vault(&tx, vault, REMOVED_BY_ADMIN)?;
        tx.commit()?;
        Ok(())
    }

    /// `account` supprime partout son coffre privé, ou le coffre commun dont il est le dernier
    /// membre, comme [`Storage::delete_vault`] ; les appareils de ses membres en suppriment leur
    /// copie.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`], [`StorageError::Gone`], [`StorageError::Deleted`] ou
    /// [`StorageError::DeletedEverywhere`] si `account` n'est pas membre,
    /// [`StorageError::SharedVault`] pour un coffre commun qui a d'autres membres : il se quitte.
    pub fn delete_own_vault(&mut self, vault: Uuid, account: Uuid) -> Result<(), StorageError> {
        let tx = self.conn.transaction_with_behavior(Immediate)?;
        if super::vaults::shared_if_member(&tx, vault, account)?
            && exists(&tx, OTHER_MEMBERS, params![vault, account])?
        {
            return Err(StorageError::SharedVault);
        }
        remove_vault(&tx, vault, REMOVED_EVERYWHERE)?;
        tx.commit()?;
        Ok(())
    }

    /// Rattache `account` à un coffre qui n'a plus de membre.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si le coffre n'existe pas, [`StorageError::UnknownAccount`] si
    /// le compte n'existe pas, [`StorageError::HasMembers`] si le coffre a encore des membres.
    pub fn attach(&mut self, vault: Uuid, account: Uuid) -> Result<(), StorageError> {
        let tx = self.conn.transaction_with_behavior(Immediate)?;
        ensure_vault_exists(&tx, vault)?;
        if !exists(&tx, ACCOUNT_EXISTS, params![account])? {
            return Err(StorageError::UnknownAccount);
        }
        if exists(&tx, HAS_MEMBERS, params![vault])? {
            return Err(StorageError::HasMembers);
        }
        tx.execute(INSERT_MEMBER, params![vault, account])?;
        tx.execute(DELETE_FORMER, params![vault, account])?;
        tx.execute(DELETE_INVITATION_TO, params![vault, account])?;
        tx.commit()?;
        Ok(())
    }

    /// Retire `account` du coffre commun ; ses appareils le voient retiré. Un coffre privé n'a
    /// que son propriétaire : il se supprime.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si le coffre n'existe pas ou si `account` n'en est pas membre,
    /// [`StorageError::NotShared`] pour un coffre privé.
    pub fn remove_member(&mut self, vault: Uuid, account: Uuid) -> Result<(), StorageError> {
        let tx = self.conn.transaction_with_behavior(Immediate)?;
        let shared: Option<bool> = tx
            .query_row(VAULT_SHARED, params![vault], |row| row.get(0))
            .optional()?;
        match shared {
            None => return Err(StorageError::NotFound),
            Some(false) => return Err(StorageError::NotShared),
            Some(true) => {}
        }
        if tx.execute(DELETE_MEMBER, params![vault, account])? == 0 {
            return Err(StorageError::NotFound);
        }
        tx.execute(INSERT_FORMER, params![vault, account])?;
        tx.commit()?;
        Ok(())
    }

    /// Versions conservées du coffre, de la plus récente à la plus ancienne.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si le coffre n'existe pas.
    pub fn versions(&self, vault: Uuid) -> Result<Vec<VersionSummary>, StorageError> {
        ensure_vault_exists(&self.conn, vault)?;
        let mut statement = self.conn.prepare(LIST_VERSIONS)?;
        let rows = statement.query_map(params![vault], version)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Crée un nouveau coffre, `<nom>-v<version>`, de même type et avec les mêmes membres, dont
    /// le contenu est cette version.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si le coffre ou la version n'existent pas.
    pub fn restore_version(
        &mut self,
        vault: Uuid,
        version: i64,
        now: Timestamp,
    ) -> Result<Restored, StorageError> {
        let tx = self.conn.transaction_with_behavior(Immediate)?;
        let (name, shared): (String, bool) = tx
            .query_row(
                "SELECT name, shared FROM vaults WHERE id = ?1 AND NOT removed",
                params![vault],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(StorageError::NotFound)?;
        let restored = Restored {
            id: Uuid::new_v4(),
            name: restored_name(&name, version),
        };
        tx.execute(INSERT_RESTORED, params![restored.id, restored.name, shared])?;
        let copy = params![restored.id, now.as_millis(), vault, version];
        if tx.execute(COPY_VERSION, copy)? == 0 {
            return Err(StorageError::NotFound);
        }
        tx.execute(COPY_MEMBERS, params![restored.id, vault])?;
        tx.commit()?;
        Ok(restored)
    }

    /// Adresse publique du serveur, renseignée par un administrateur.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn public_address(&self) -> Result<Option<String>, StorageError> {
        let address = self.conn.query_row(
            "SELECT public_address FROM settings WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(address)
    }

    /// # Errors
    ///
    /// Si l'écriture dans la base échoue.
    pub fn set_public_address(&mut self, address: Option<&str>) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE settings SET public_address = ?1 WHERE id = 1",
            params![address],
        )?;
        Ok(())
    }

    /// Copie cohérente de toute la base, certificat compris : un fichier SQLite complet.
    ///
    /// # Errors
    ///
    /// Si SQLite ne peut pas copier la base.
    pub fn backup(&mut self) -> Result<Vec<u8>, StorageError> {
        Ok(self.conn.serialize(MAIN_DB)?.to_vec())
    }
}

fn ensure_not_last_admin(conn: &Connection, account: Uuid) -> Result<(), StorageError> {
    let (is_admin, admins): (bool, i64) = conn
        .query_row(ADMIN_STATUS, params![account], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?
        .ok_or(StorageError::NotFound)?;
    if is_admin && admins == 1 {
        return Err(StorageError::LastAdmin);
    }
    Ok(())
}

fn ensure_vault_exists(conn: &Connection, vault: Uuid) -> Result<(), StorageError> {
    if exists(conn, VAULT_EXISTS, params![vault])? {
        Ok(())
    } else {
        Err(StorageError::NotFound)
    }
}

fn remove_vault(conn: &Connection, vault: Uuid, removed: i64) -> Result<(), StorageError> {
    for statement in REMOVE_VAULT {
        conn.execute(statement, params![vault])?;
    }
    conn.execute(MARK_REMOVED, params![vault, removed])?;
    Ok(())
}

/// `<nom>-v<version>`, le nom raccourci pour rester un nom de coffre valide.
fn restored_name(name: &str, version: i64) -> String {
    let suffix = format!("-v{version}");
    let kept = MAX_VAULT_NAME_LEN.saturating_sub(suffix.len());
    name.chars().take(kept).collect::<String>() + &suffix
}

/// Identifiant, nom et rôle d'un compte.
fn account_row(row: &Row<'_>) -> rusqlite::Result<(Uuid, String, bool)> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
}

fn device(row: &Row<'_>) -> rusqlite::Result<DeviceSummary> {
    Ok(DeviceSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at: Timestamp::from_millis(row.get(2)?),
    })
}

fn pending_key(row: &Row<'_>) -> rusqlite::Result<PendingKey> {
    let member: Option<String> = row.get(1)?;
    let account: Option<String> = row.get(2)?;
    let purpose = match (member, account) {
        (Some(name), _) => PendingPurpose::Member { name },
        (None, Some(account)) => PendingPurpose::Device { account },
        (None, None) => PendingPurpose::Admin,
    };
    Ok(PendingKey {
        id: row.get(0)?,
        purpose,
        expires_at: Timestamp::from_millis(row.get(3)?),
    })
}

fn version(row: &Row<'_>) -> rusqlite::Result<VersionSummary> {
    Ok(VersionSummary {
        version: row.get(0)?,
        created_at: Timestamp::from_millis(row.get(1)?),
        size: row.get(2)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{KeyPurpose, NewDevice};

    const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);
    const LATER: Timestamp = Timestamp::from_millis(i64::MAX);

    /// Arthur (administrateur), Camille et Dominique ; Arthur et Camille partagent `famille`.
    struct Base {
        storage: Storage,
        arthur: Uuid,
        camille: Uuid,
        dominique: Uuid,
        famille: Uuid,
    }

    fn connect(storage: &mut Storage, secret: &str, purpose: &KeyPurpose, name: &str) -> Uuid {
        storage
            .add_connection_key(secret.as_bytes(), purpose, LATER)
            .unwrap();
        let device = NewDevice {
            name: "PC",
            token_hash: secret.as_bytes(),
            account_name: Some(name),
        };
        storage
            .redeem_connection_key(secret.as_bytes(), NOW, &device)
            .unwrap()
            .account
    }

    fn member(storage: &mut Storage, name: &str) -> Uuid {
        let purpose = KeyPurpose::Member {
            name: name.to_owned(),
        };
        connect(storage, name, &purpose, name)
    }

    fn base() -> Base {
        let mut storage = Storage::open_in_memory().unwrap();
        let arthur = connect(&mut storage, "Arthur", &KeyPurpose::Admin, "Arthur");
        let camille = member(&mut storage, "Camille");
        let dominique = member(&mut storage, "Dominique");
        let famille = storage.create_vault(arthur, "famille", true).unwrap();
        let invitation = storage.invite(famille, arthur, camille, NOW).unwrap();
        storage.accept_invitation(invitation, camille).unwrap();
        Base {
            storage,
            arthur,
            camille,
            dominique,
            famille,
        }
    }

    fn summary(id: Uuid, name: &str) -> AccountSummary {
        AccountSummary {
            id,
            name: name.to_owned(),
        }
    }

    fn count(storage: &Storage, query: &str, id: Uuid) -> i64 {
        storage
            .conn
            .query_row(query, params![id], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn accounts_are_listed_with_their_devices() {
        let mut base = base();
        let purpose = KeyPurpose::Device {
            account: base.camille,
        };
        connect(&mut base.storage, "Camille-2", &purpose, "ignoré");
        let accounts = base.storage.account_details().unwrap();
        let summary: Vec<(&str, bool, usize)> = accounts
            .iter()
            .map(|a| (a.name.as_str(), a.is_admin, a.devices.len()))
            .collect();
        assert_eq!(
            summary,
            [
                ("Arthur", true, 1),
                ("Camille", false, 2),
                ("Dominique", false, 1)
            ]
        );
        let device = &accounts[0].devices[0];
        assert_eq!((device.name.as_str(), device.created_at), ("PC", NOW));
    }

    #[test]
    fn a_deleted_account_leaves_its_vaults_behind() {
        let mut base = base();
        let perso = base
            .storage
            .create_vault(base.camille, "perso", false)
            .unwrap();
        base.storage
            .invite(base.famille, base.camille, base.dominique, NOW)
            .unwrap();
        let purpose = KeyPurpose::Device {
            account: base.camille,
        };
        base.storage
            .add_connection_key(b"cle", &purpose, LATER)
            .unwrap();

        base.storage.delete_account(base.camille).unwrap();
        assert_eq!(base.storage.authenticate(b"Camille").unwrap(), None);
        assert_eq!(base.storage.pending_keys(NOW).unwrap(), []);
        assert_eq!(
            base.storage.received_invitations(base.dominique).unwrap(),
            []
        );
        let vaults = base.storage.server_vaults().unwrap();
        assert_eq!(vaults[0].members, [summary(base.arthur, "Arthur")]);
        assert_eq!((vaults[1].id, vaults[1].members.len()), (perso, 0));
        assert!(matches!(
            base.storage.delete_account(base.camille),
            Err(StorageError::NotFound)
        ));
    }

    #[test]
    fn the_last_administrator_stays() {
        let mut base = base();
        assert!(matches!(
            base.storage.delete_account(base.arthur),
            Err(StorageError::LastAdmin)
        ));
        assert!(matches!(
            base.storage.set_admin(base.arthur, false),
            Err(StorageError::LastAdmin)
        ));
        base.storage.set_admin(base.camille, true).unwrap();
        base.storage.set_admin(base.arthur, false).unwrap();
        assert!(
            !base
                .storage
                .authenticate(b"Arthur")
                .unwrap()
                .unwrap()
                .is_admin
        );
        assert!(
            base.storage
                .authenticate(b"Camille")
                .unwrap()
                .unwrap()
                .is_admin
        );
        base.storage.set_admin(base.dominique, false).unwrap();
        base.storage.delete_account(base.arthur).unwrap();
        assert!(matches!(
            base.storage.set_admin(Uuid::new_v4(), true),
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            base.storage.set_admin(Uuid::new_v4(), false),
            Err(StorageError::NotFound)
        ));
    }

    #[test]
    fn pending_keys_are_listed_then_cancelled() {
        let mut base = base();
        let soon = Timestamp::from_millis(i64::MAX - 1);
        let member = KeyPurpose::Member {
            name: "Élise".to_owned(),
        };
        let device = KeyPurpose::Device {
            account: base.camille,
        };
        base.storage
            .add_connection_key(b"admin", &KeyPurpose::Admin, LATER)
            .unwrap();
        base.storage
            .add_connection_key(b"membre", &member, soon)
            .unwrap();
        base.storage
            .add_connection_key(b"appareil", &device, LATER)
            .unwrap();
        base.storage
            .add_connection_key(b"vieille", &device, NOW)
            .unwrap();

        let keys = base.storage.pending_keys(NOW).unwrap();
        let purposes: Vec<&PendingPurpose> = keys.iter().map(|key| &key.purpose).collect();
        assert_eq!(purposes.len(), 3);
        assert_eq!(
            *purposes[0],
            PendingPurpose::Member {
                name: "Élise".to_owned()
            }
        );
        assert_eq!(keys[0].expires_at, soon);
        assert!(purposes.contains(&&PendingPurpose::Admin));
        assert!(purposes.contains(&&PendingPurpose::Device {
            account: "Camille".to_owned()
        }));

        base.storage.cancel_key(keys[0].id).unwrap();
        assert!(matches!(
            base.storage.cancel_key(keys[0].id),
            Err(StorageError::NotFound)
        ));
        assert_eq!(base.storage.pending_keys(NOW).unwrap().len(), 2);
        let redeemed = NewDevice {
            name: "PC",
            token_hash: b"jeton",
            account_name: None,
        };
        assert!(matches!(
            base.storage
                .redeem_connection_key(b"membre", NOW, &redeemed),
            Err(StorageError::InvalidKey)
        ));
    }

    #[test]
    fn administrators_see_every_vault() {
        let mut base = base();
        let perso = base
            .storage
            .create_vault(base.dominique, "perso", false)
            .unwrap();
        base.storage
            .publish(perso, base.dominique, b"x", None, NOW)
            .unwrap();
        assert_eq!(
            base.storage.server_vaults().unwrap(),
            [
                ServerVault {
                    id: base.famille,
                    name: "famille".to_owned(),
                    shared: true,
                    version: None,
                    members: vec![
                        summary(base.arthur, "Arthur"),
                        summary(base.camille, "Camille")
                    ],
                },
                ServerVault {
                    id: perso,
                    name: "perso".to_owned(),
                    shared: false,
                    version: Some(1),
                    members: vec![summary(base.dominique, "Dominique")],
                },
            ]
        );
    }

    #[test]
    fn a_deleted_vault_is_erased_and_gone_for_its_members() {
        let mut base = base();
        base.storage
            .publish(base.famille, base.arthur, b"x", None, NOW)
            .unwrap();
        base.storage
            .invite(base.famille, base.arthur, base.dominique, NOW)
            .unwrap();
        base.storage.delete_vault(base.famille).unwrap();

        for account in [base.arthur, base.camille] {
            assert!(matches!(
                base.storage.fetch(base.famille, account),
                Err(StorageError::Deleted)
            ));
        }
        assert_eq!(base.storage.list_vaults(base.arthur).unwrap(), []);
        assert_eq!(base.storage.server_vaults().unwrap(), []);
        assert_eq!(
            base.storage.received_invitations(base.dominique).unwrap(),
            []
        );
        let versions = "SELECT COUNT(*) FROM vault_versions WHERE vault_id = ?1";
        assert_eq!(count(&base.storage, versions, base.famille), 0);
        assert!(matches!(
            base.storage.delete_vault(base.famille),
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            base.storage.versions(base.famille),
            Err(StorageError::NotFound)
        ));
    }

    #[test]
    fn a_member_deletes_a_private_vault_everywhere() {
        let mut base = base();
        let perso = base
            .storage
            .create_vault(base.camille, "perso", false)
            .unwrap();
        assert!(matches!(
            base.storage.delete_own_vault(perso, base.arthur),
            Err(StorageError::NotFound)
        ));
        base.storage.delete_own_vault(perso, base.camille).unwrap();
        assert!(matches!(
            base.storage.fetch(perso, base.camille),
            Err(StorageError::DeletedEverywhere)
        ));
        assert!(matches!(
            base.storage.delete_own_vault(perso, base.camille),
            Err(StorageError::DeletedEverywhere)
        ));
    }

    #[test]
    fn only_the_last_member_deletes_a_shared_vault() {
        let mut base = base();
        assert!(matches!(
            base.storage.delete_own_vault(base.famille, base.camille),
            Err(StorageError::SharedVault)
        ));
        base.storage.leave(base.famille, base.arthur).unwrap();
        let invitation = base
            .storage
            .invite(base.famille, base.camille, base.dominique, NOW)
            .unwrap();
        base.storage
            .delete_own_vault(base.famille, base.camille)
            .unwrap();
        for account in [base.arthur, base.camille] {
            assert!(matches!(
                base.storage.fetch(base.famille, account),
                Err(StorageError::DeletedEverywhere)
            ));
        }
        assert!(matches!(
            base.storage.accept_invitation(invitation, base.dominique),
            Err(StorageError::NotFound)
        ));
    }

    #[test]
    fn an_account_is_attached_only_to_a_vault_without_member() {
        let mut base = base();
        assert!(matches!(
            base.storage.attach(base.famille, base.dominique),
            Err(StorageError::HasMembers)
        ));
        base.storage.leave(base.famille, base.camille).unwrap();
        base.storage
            .invite(base.famille, base.arthur, base.camille, NOW)
            .unwrap();
        base.storage.leave(base.famille, base.arthur).unwrap();
        assert!(matches!(
            base.storage.attach(base.famille, Uuid::new_v4()),
            Err(StorageError::UnknownAccount)
        ));
        assert!(matches!(
            base.storage.attach(Uuid::new_v4(), base.camille),
            Err(StorageError::NotFound)
        ));

        base.storage.attach(base.famille, base.camille).unwrap();
        assert!(base.storage.fetch(base.famille, base.camille).is_ok());
        assert_eq!(base.storage.received_invitations(base.camille).unwrap(), []);
        assert!(matches!(
            base.storage.fetch(base.famille, base.arthur),
            Err(StorageError::Gone)
        ));
    }

    #[test]
    fn a_removed_member_sees_the_vault_gone() {
        let mut base = base();
        base.storage
            .remove_member(base.famille, base.camille)
            .unwrap();
        assert!(matches!(
            base.storage.fetch(base.famille, base.camille),
            Err(StorageError::Gone)
        ));
        assert!(matches!(
            base.storage.remove_member(base.famille, base.camille),
            Err(StorageError::NotFound)
        ));
        assert!(matches!(
            base.storage.remove_member(Uuid::new_v4(), base.arthur),
            Err(StorageError::NotFound)
        ));
        base.storage.delete_vault(base.famille).unwrap();
        assert!(matches!(
            base.storage.fetch(base.famille, base.camille),
            Err(StorageError::Deleted)
        ));
    }

    #[test]
    fn the_owner_of_a_private_vault_cannot_be_removed() {
        let mut base = base();
        let perso = base
            .storage
            .create_vault(base.camille, "perso", false)
            .unwrap();
        assert!(matches!(
            base.storage.remove_member(perso, base.camille),
            Err(StorageError::NotShared)
        ));
        assert_eq!(base.storage.fetch(perso, base.camille).unwrap(), None);
    }

    #[test]
    fn versions_are_listed_newest_first() {
        let mut base = base();
        assert_eq!(base.storage.versions(base.famille).unwrap(), []);
        let first = base
            .storage
            .publish(base.famille, base.arthur, b"a", None, NOW)
            .unwrap();
        let later = Timestamp::from_millis(NOW.as_millis() + 1);
        base.storage
            .publish(base.famille, base.camille, b"bc", Some(first), later)
            .unwrap();
        assert_eq!(
            base.storage.versions(base.famille).unwrap(),
            [
                VersionSummary {
                    version: 2,
                    created_at: later,
                    size: 2
                },
                VersionSummary {
                    version: 1,
                    created_at: NOW,
                    size: 1
                },
            ]
        );
    }

    #[test]
    fn a_restored_version_is_a_new_vault_with_the_same_members() {
        let mut base = base();
        let first = base
            .storage
            .publish(base.famille, base.arthur, b"ancien", None, NOW)
            .unwrap();
        base.storage
            .publish(base.famille, base.arthur, b"nouveau", Some(first), NOW)
            .unwrap();
        let later = Timestamp::from_millis(NOW.as_millis() + 1);
        let restored = base
            .storage
            .restore_version(base.famille, first, later)
            .unwrap();
        assert_eq!(restored.name, "famille-v1");
        for account in [base.arthur, base.camille] {
            let file = base.storage.fetch(restored.id, account).unwrap().unwrap();
            assert_eq!((file.content.as_slice(), file.version), (&b"ancien"[..], 1));
        }
        let listed = base.storage.server_vaults().unwrap();
        assert!(
            listed
                .iter()
                .any(|vault| vault.id == restored.id && vault.shared)
        );
        assert_eq!(
            base.storage.versions(restored.id).unwrap()[0].created_at,
            later
        );

        for (vault, version) in [(base.famille, 9), (Uuid::new_v4(), 1)] {
            assert!(matches!(
                base.storage.restore_version(vault, version, NOW),
                Err(StorageError::NotFound)
            ));
        }
        assert_eq!(base.storage.server_vaults().unwrap().len(), 2);
    }

    #[test]
    fn restored_names_stay_valid() {
        assert_eq!(restored_name("perso", 12), "perso-v12");
        let long = "a".repeat(MAX_VAULT_NAME_LEN);
        let name = restored_name(&long, 3);
        assert_eq!(name.len(), MAX_VAULT_NAME_LEN);
        assert!(name.ends_with("a-v3"));
        assert!(pswd_core::is_valid_vault_name(&name));
    }

    #[test]
    fn public_address_is_kept() {
        let mut storage = Storage::open_in_memory().unwrap();
        assert_eq!(storage.public_address().unwrap(), None);
        storage
            .set_public_address(Some("maison.example.org"))
            .unwrap();
        assert_eq!(
            storage.public_address().unwrap().as_deref(),
            Some("maison.example.org")
        );
        storage.set_public_address(None).unwrap();
        assert_eq!(storage.public_address().unwrap(), None);
    }

    #[test]
    fn backup_is_a_complete_database_without_deleted_content() {
        let mut base = base();
        let marker = b"contenu-du-coffre-supprime";
        base.storage
            .publish(base.famille, base.arthur, marker, None, NOW)
            .unwrap();
        base.storage.delete_vault(base.famille).unwrap();
        base.storage.set_public_address(Some("maison")).unwrap();

        let copy = base.storage.backup().unwrap();
        assert!(!copy.windows(marker.len()).any(|window| window == marker));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("copie.db");
        std::fs::write(&path, &copy).unwrap();
        let restored = Storage::open(&path).unwrap();
        assert_eq!(restored.account_details().unwrap().len(), 3);
        assert_eq!(
            restored.public_address().unwrap().as_deref(),
            Some("maison")
        );
    }

    #[test]
    fn database_failures_are_reported() {
        let mut base = base();
        let famille = base.famille;
        base.storage.break_for_tests("vault_versions");
        assert!(matches!(
            base.storage.versions(famille),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.restore_version(famille, 1, NOW),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.delete_vault(famille),
            Err(StorageError::Database(_))
        ));
        base.storage.break_for_tests("former_members");
        assert!(matches!(
            base.storage.remove_member(famille, base.camille),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.delete_account(base.camille),
            Err(StorageError::Database(_))
        ));
        base.storage.break_for_tests("devices");
        assert!(matches!(
            base.storage.account_details(),
            Err(StorageError::Database(_))
        ));
        base.storage.break_for_tests("vault_members");
        for result in [
            base.storage.attach(famille, base.dominique),
            base.storage.delete_own_vault(famille, base.arthur),
        ] {
            assert!(matches!(result, Err(StorageError::Database(_))));
        }
        assert!(matches!(
            base.storage.server_vaults(),
            Err(StorageError::Database(_))
        ));
        base.storage.break_for_tests("connection_keys");
        assert!(matches!(
            base.storage.pending_keys(NOW),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.cancel_key(Uuid::new_v4()),
            Err(StorageError::Database(_))
        ));
        base.storage.break_for_tests("settings");
        assert!(matches!(
            base.storage.public_address(),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.set_public_address(None),
            Err(StorageError::Database(_))
        ));
        base.storage.break_for_tests("accounts");
        for result in [
            base.storage.set_admin(base.camille, true),
            base.storage.set_admin(base.camille, false),
            base.storage.delete_account(base.camille),
        ] {
            assert!(matches!(result, Err(StorageError::Database(_))));
        }
        base.storage.break_for_tests("vaults");
        assert!(matches!(
            base.storage.server_vaults(),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.remove_member(famille, base.camille),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.restore_version(famille, 1, NOW),
            Err(StorageError::Database(_))
        ));
    }

    #[test]
    fn unreadable_rows_are_database_errors() {
        let base = base();
        let corrupt = |query: &str| {
            base.storage.conn.execute(query, []).unwrap();
        };
        corrupt("UPDATE devices SET created_at = 'abc'");
        assert!(matches!(
            base.storage.account_details(),
            Err(StorageError::Database(_))
        ));
        corrupt("UPDATE accounts SET name = X'FF'");
        assert!(matches!(
            base.storage.account_details(),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.server_vaults(),
            Err(StorageError::Database(_))
        ));
        corrupt("UPDATE vaults SET current_version = 'abc'");
        assert!(matches!(
            base.storage.server_vaults(),
            Err(StorageError::Database(_))
        ));
    }
}
