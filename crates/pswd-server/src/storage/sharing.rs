//! Coffres communs : comptes à inviter, invitations, membres et départs.

use pswd_core::Timestamp;
use rusqlite::{Connection, OptionalExtension, Params, Row, params};
use uuid::Uuid;

use super::vaults::shared_if_member;
use super::{Storage, StorageError};

const LIST_ACCOUNTS: &str = "SELECT id, name FROM accounts ORDER BY name, id";
pub(super) const ACCOUNT_EXISTS: &str = "SELECT EXISTS (SELECT 1 FROM accounts WHERE id = ?1)";
const MEMBER_OR_INVITED: &str = "SELECT EXISTS (
         SELECT 1 FROM vault_members WHERE vault_id = ?1 AND account_id = ?2
     ) OR EXISTS (SELECT 1 FROM invitations WHERE vault_id = ?1 AND account_id = ?2)";
const INSERT_INVITATION: &str = "INSERT INTO invitations
     (id, vault_id, account_id, invited_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5)";
const INVITED_VAULT: &str = "SELECT vault_id FROM invitations WHERE id = ?1 AND account_id = ?2";
pub(super) const INSERT_MEMBER: &str =
    "INSERT INTO vault_members (vault_id, account_id) VALUES (?1, ?2)";
pub(super) const DELETE_MEMBER: &str =
    "DELETE FROM vault_members WHERE vault_id = ?1 AND account_id = ?2";
pub(super) const INSERT_FORMER: &str =
    "INSERT INTO former_members (vault_id, account_id) VALUES (?1, ?2)";
pub(super) const DELETE_FORMER: &str =
    "DELETE FROM former_members WHERE vault_id = ?1 AND account_id = ?2";
/// Membres d'un coffre, triés par nom.
pub(super) const LIST_MEMBERS: &str = "SELECT a.id, a.name FROM accounts a
     JOIN vault_members m ON m.account_id = a.id
     WHERE m.vault_id = ?1 ORDER BY a.name, a.id";
const DELETE_INVITATION: &str = "DELETE FROM invitations WHERE id = ?1";
/// Seuls l'invité et les membres du coffre suppriment une invitation.
const DELETE_INVITATION_OF: &str = "DELETE FROM invitations
     WHERE id = ?1 AND (account_id = ?2 OR EXISTS (
         SELECT 1 FROM vault_members m
         WHERE m.vault_id = invitations.vault_id AND m.account_id = ?2))";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountSummary {
    pub id: Uuid,
    pub name: String,
}

/// Invitation en attente, vue par les membres du coffre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentInvitation {
    pub id: Uuid,
    pub account: AccountSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Members {
    pub members: Vec<AccountSummary>,
    pub invitations: Vec<SentInvitation>,
}

/// Invitation reçue par un compte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceivedInvitation {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub vault_name: String,
    /// Nom du compte qui a invité.
    pub invited_by: String,
}

impl Storage {
    /// Comptes du serveur, triés par nom.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn list_accounts(&self) -> Result<Vec<AccountSummary>, StorageError> {
        let mut statement = self.conn.prepare(LIST_ACCOUNTS)?;
        let rows = statement.query_map([], account)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Membres du coffre et invitations en attente, triés par nom.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`], [`StorageError::Gone`] ou [`StorageError::Deleted`] si `account`
    /// n'est pas membre.
    pub fn members(&self, vault: Uuid, account: Uuid) -> Result<Members, StorageError> {
        shared_if_member(&self.conn, vault, account)?;
        let mut statement = self.conn.prepare(LIST_MEMBERS)?;
        let members = statement
            .query_map(params![vault], self::account)?
            .collect::<Result<_, _>>()?;
        let mut statement = self.conn.prepare(
            "SELECT i.id, a.id, a.name FROM invitations i
             JOIN accounts a ON a.id = i.account_id
             WHERE i.vault_id = ?1 ORDER BY a.name, a.id",
        )?;
        let rows = statement.query_map(params![vault], sent)?;
        let invitations = rows.collect::<Result<_, _>>()?;
        Ok(Members {
            members,
            invitations,
        })
    }

    /// `account`, membre du coffre commun, invite `invitee` ; renvoie l'identifiant de
    /// l'invitation.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`], [`StorageError::Gone`] ou [`StorageError::Deleted`] si `account`
    /// n'est pas membre,
    /// [`StorageError::NotShared`] pour un coffre privé, [`StorageError::UnknownAccount`] si
    /// `invitee` n'existe pas, [`StorageError::AlreadyMember`] s'il est déjà membre ou invité.
    pub fn invite(
        &mut self,
        vault: Uuid,
        account: Uuid,
        invitee: Uuid,
        now: Timestamp,
    ) -> Result<Uuid, StorageError> {
        let tx = self.conn.transaction()?;
        if !shared_if_member(&tx, vault, account)? {
            return Err(StorageError::NotShared);
        }
        if !exists(&tx, ACCOUNT_EXISTS, params![invitee])? {
            return Err(StorageError::UnknownAccount);
        }
        if exists(&tx, MEMBER_OR_INVITED, params![vault, invitee])? {
            return Err(StorageError::AlreadyMember);
        }
        let id = Uuid::new_v4();
        let invitation = params![id, vault, invitee, account, now.as_millis()];
        tx.execute(INSERT_INVITATION, invitation)?;
        tx.commit()?;
        Ok(id)
    }

    /// Invitations reçues par `account`, des plus anciennes aux plus récentes.
    ///
    /// # Errors
    ///
    /// Si la lecture de la base échoue.
    pub fn received_invitations(
        &self,
        account: Uuid,
    ) -> Result<Vec<ReceivedInvitation>, StorageError> {
        let mut statement = self.conn.prepare(
            "SELECT i.id, v.id, v.name, a.name FROM invitations i
             JOIN vaults v ON v.id = i.vault_id
             JOIN accounts a ON a.id = i.invited_by
             WHERE i.account_id = ?1 ORDER BY i.created_at, i.id",
        )?;
        let invitations = statement
            .query_map(params![account], received)?
            .collect::<Result<_, _>>()?;
        Ok(invitations)
    }

    /// `account` accepte l'invitation : il devient membre du coffre, dont l'identifiant est
    /// renvoyé ; une trace d'un départ précédent est effacée.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si l'invitation n'existe pas ou ne lui est pas adressée.
    pub fn accept_invitation(
        &mut self,
        invitation: Uuid,
        account: Uuid,
    ) -> Result<Uuid, StorageError> {
        let tx = self.conn.transaction()?;
        let vault: Uuid = tx
            .query_row(INVITED_VAULT, params![invitation, account], |row| {
                row.get(0)
            })
            .optional()?
            .ok_or(StorageError::NotFound)?;
        tx.execute(INSERT_MEMBER, params![vault, account])?;
        tx.execute(DELETE_FORMER, params![vault, account])?;
        tx.execute(DELETE_INVITATION, params![invitation])?;
        tx.commit()?;
        Ok(vault)
    }

    /// Supprime l'invitation : refusée par l'invité, ou annulée par un membre du coffre.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`] si l'invitation n'existe pas, ou si `account` n'est ni l'invité
    /// ni un membre du coffre.
    pub fn delete_invitation(
        &mut self,
        invitation: Uuid,
        account: Uuid,
    ) -> Result<(), StorageError> {
        let deleted = self
            .conn
            .execute(DELETE_INVITATION_OF, params![invitation, account])?;
        if deleted == 0 {
            return Err(StorageError::NotFound);
        }
        Ok(())
    }

    /// `account` quitte le coffre commun ; ses appareils le verront retiré.
    ///
    /// # Errors
    ///
    /// [`StorageError::NotFound`], [`StorageError::Gone`] ou [`StorageError::Deleted`] si `account`
    /// n'est pas membre,
    /// [`StorageError::NotShared`] pour un coffre privé.
    pub fn leave(&mut self, vault: Uuid, account: Uuid) -> Result<(), StorageError> {
        let tx = self.conn.transaction()?;
        if !shared_if_member(&tx, vault, account)? {
            return Err(StorageError::NotShared);
        }
        tx.execute(DELETE_MEMBER, params![vault, account])?;
        tx.execute(INSERT_FORMER, params![vault, account])?;
        tx.commit()?;
        Ok(())
    }
}

pub(super) fn exists(
    conn: &Connection,
    query: &str,
    args: impl Params,
) -> Result<bool, StorageError> {
    Ok(conn.query_row(query, args, |row| row.get(0))?)
}

pub(super) fn account(row: &Row<'_>) -> rusqlite::Result<AccountSummary> {
    Ok(AccountSummary {
        id: row.get(0)?,
        name: row.get(1)?,
    })
}

fn sent(row: &Row<'_>) -> rusqlite::Result<SentInvitation> {
    Ok(SentInvitation {
        id: row.get(0)?,
        account: AccountSummary {
            id: row.get(1)?,
            name: row.get(2)?,
        },
    })
}

fn received(row: &Row<'_>) -> rusqlite::Result<ReceivedInvitation> {
    Ok(ReceivedInvitation {
        id: row.get(0)?,
        vault_id: row.get(1)?,
        vault_name: row.get(2)?,
        invited_by: row.get(3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{KeyPurpose, NewDevice};

    const NOW: Timestamp = Timestamp::from_millis(1_700_000_000_000);

    struct Base {
        storage: Storage,
        arthur: Uuid,
        camille: Uuid,
        dominique: Uuid,
        /// Coffre commun d'Arthur.
        famille: Uuid,
    }

    fn account_named(storage: &mut Storage, name: &str) -> Uuid {
        let purpose = KeyPurpose::Member {
            name: name.to_owned(),
        };
        storage
            .add_connection_key(name.as_bytes(), &purpose, Timestamp::from_millis(i64::MAX))
            .unwrap();
        let device = NewDevice {
            name: "PC",
            token_hash: name.as_bytes(),
            account_name: None,
        };
        storage
            .redeem_connection_key(name.as_bytes(), NOW, &device)
            .unwrap()
            .account
    }

    fn base() -> Base {
        let mut storage = Storage::open_in_memory().unwrap();
        let camille = account_named(&mut storage, "Camille");
        let arthur = account_named(&mut storage, "Arthur");
        let dominique = account_named(&mut storage, "Dominique");
        let famille = storage.create_vault(arthur, "famille", true).unwrap();
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

    #[test]
    fn accounts_are_listed_by_name() {
        let base = base();
        let names: Vec<String> = base
            .storage
            .list_accounts()
            .unwrap()
            .into_iter()
            .map(|account| account.name)
            .collect();
        assert_eq!(names, ["Arthur", "Camille", "Dominique"]);
    }

    #[test]
    fn an_accepted_invitation_makes_a_member() {
        let mut base = base();
        let id = base
            .storage
            .invite(base.famille, base.arthur, base.camille, NOW)
            .unwrap();
        assert_eq!(
            base.storage.members(base.famille, base.arthur).unwrap(),
            Members {
                members: vec![summary(base.arthur, "Arthur")],
                invitations: vec![SentInvitation {
                    id,
                    account: summary(base.camille, "Camille"),
                }],
            }
        );
        assert_eq!(
            base.storage.received_invitations(base.camille).unwrap(),
            [ReceivedInvitation {
                id,
                vault_id: base.famille,
                vault_name: "famille".to_owned(),
                invited_by: "Arthur".to_owned(),
            }]
        );
        assert!(matches!(
            base.storage.accept_invitation(id, base.dominique),
            Err(StorageError::NotFound)
        ));
        assert_eq!(
            base.storage.accept_invitation(id, base.camille).unwrap(),
            base.famille
        );
        assert_eq!(base.storage.received_invitations(base.camille).unwrap(), []);
        let members = base.storage.members(base.famille, base.camille).unwrap();
        assert_eq!(
            members.members,
            [
                summary(base.arthur, "Arthur"),
                summary(base.camille, "Camille")
            ]
        );
        assert_eq!(members.invitations, []);
        assert!(base.storage.fetch(base.famille, base.camille).is_ok());
    }

    #[test]
    fn only_members_of_a_shared_vault_invite_new_accounts() {
        let mut base = base();
        let mut invite =
            |by: Uuid, invitee: Uuid| base.storage.invite(base.famille, by, invitee, NOW).err();
        assert!(matches!(
            invite(base.camille, base.dominique),
            Some(StorageError::NotFound)
        ));
        assert!(matches!(
            invite(base.arthur, Uuid::new_v4()),
            Some(StorageError::UnknownAccount)
        ));
        assert!(matches!(
            invite(base.arthur, base.arthur),
            Some(StorageError::AlreadyMember)
        ));
        assert!(invite(base.arthur, base.camille).is_none());
        assert!(matches!(
            invite(base.arthur, base.camille),
            Some(StorageError::AlreadyMember)
        ));
        let private = base
            .storage
            .create_vault(base.arthur, "perso", false)
            .unwrap();
        assert!(matches!(
            base.storage.invite(private, base.arthur, base.camille, NOW),
            Err(StorageError::NotShared)
        ));
        assert!(matches!(
            base.storage.members(base.famille, base.dominique),
            Err(StorageError::NotFound)
        ));
    }

    #[test]
    fn invitee_refuses_and_members_cancel() {
        let mut base = base();
        let refused = base
            .storage
            .invite(base.famille, base.arthur, base.camille, NOW)
            .unwrap();
        let cancelled = base
            .storage
            .invite(base.famille, base.arthur, base.dominique, NOW)
            .unwrap();
        assert!(matches!(
            base.storage.delete_invitation(refused, base.dominique),
            Err(StorageError::NotFound)
        ));
        base.storage
            .delete_invitation(refused, base.camille)
            .unwrap();
        base.storage
            .delete_invitation(cancelled, base.arthur)
            .unwrap();
        assert!(matches!(
            base.storage.delete_invitation(cancelled, base.arthur),
            Err(StorageError::NotFound)
        ));
        assert_eq!(
            base.storage
                .members(base.famille, base.arthur)
                .unwrap()
                .invitations,
            []
        );
        assert_eq!(base.storage.received_invitations(base.camille).unwrap(), []);
    }

    #[test]
    fn a_member_who_left_sees_the_vault_gone_until_invited_again() {
        let mut base = base();
        let id = base
            .storage
            .invite(base.famille, base.arthur, base.camille, NOW)
            .unwrap();
        base.storage.accept_invitation(id, base.camille).unwrap();
        base.storage.leave(base.famille, base.camille).unwrap();
        assert!(matches!(
            base.storage.fetch(base.famille, base.camille),
            Err(StorageError::Gone)
        ));
        assert!(matches!(
            base.storage
                .publish(base.famille, base.camille, b"x", None, NOW),
            Err(StorageError::Gone)
        ));
        assert!(matches!(
            base.storage.leave(base.famille, base.camille),
            Err(StorageError::Gone)
        ));
        assert!(matches!(
            base.storage.fetch(base.famille, base.dominique),
            Err(StorageError::NotFound)
        ));

        let id = base
            .storage
            .invite(base.famille, base.arthur, base.camille, NOW)
            .unwrap();
        base.storage.accept_invitation(id, base.camille).unwrap();
        assert!(base.storage.fetch(base.famille, base.camille).is_ok());
    }

    #[test]
    fn a_private_vault_cannot_be_left() {
        let mut base = base();
        let private = base
            .storage
            .create_vault(base.arthur, "perso", false)
            .unwrap();
        assert!(matches!(
            base.storage.leave(private, base.arthur),
            Err(StorageError::NotShared)
        ));
        assert!(base.storage.fetch(private, base.arthur).is_ok());
    }

    #[test]
    fn failed_acceptance_changes_nothing() {
        let mut base = base();
        let id = base
            .storage
            .invite(base.famille, base.arthur, base.camille, NOW)
            .unwrap();
        base.storage.break_for_tests("vault_members");
        assert!(matches!(
            base.storage.accept_invitation(id, base.camille),
            Err(StorageError::Database(_))
        ));
        assert_eq!(
            base.storage
                .received_invitations(base.camille)
                .unwrap()
                .len(),
            1
        );
    }

    /// Remplace le nom du compte par des octets qui ne sont pas du texte.
    fn corrupt_name(storage: &Storage, account: Uuid) {
        storage
            .conn
            .execute(
                "UPDATE accounts SET name = X'FF' WHERE id = ?1",
                params![account],
            )
            .unwrap();
    }

    #[test]
    fn unreadable_rows_are_database_errors() {
        let mut base = base();
        base.storage
            .invite(base.famille, base.arthur, base.camille, NOW)
            .unwrap();
        corrupt_name(&base.storage, base.camille);
        assert!(matches!(
            base.storage.list_accounts(),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.members(base.famille, base.arthur),
            Err(StorageError::Database(_))
        ));
        corrupt_name(&base.storage, base.arthur);
        assert!(matches!(
            base.storage.members(base.famille, base.arthur),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.received_invitations(base.camille),
            Err(StorageError::Database(_))
        ));
    }

    #[test]
    fn unreadable_membership_is_a_database_error() {
        let base = base();
        base.storage.break_for_tests("vault_members");
        assert!(matches!(
            base.storage.members(base.famille, base.arthur),
            Err(StorageError::Database(_))
        ));
    }

    #[test]
    fn database_failures_are_reported() {
        let mut base = base();
        base.storage.break_for_tests("invitations");
        assert!(matches!(
            base.storage
                .invite(base.famille, base.arthur, base.camille, NOW),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.members(base.famille, base.arthur),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.received_invitations(base.camille),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.accept_invitation(Uuid::new_v4(), base.camille),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.delete_invitation(Uuid::new_v4(), base.camille),
            Err(StorageError::Database(_))
        ));
        base.storage.break_for_tests("former_members");
        assert!(matches!(
            base.storage.leave(base.famille, base.arthur),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.fetch(base.famille, base.camille),
            Err(StorageError::Database(_))
        ));
        base.storage.break_for_tests("accounts");
        assert!(matches!(
            base.storage.list_accounts(),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage.members(base.famille, base.arthur),
            Err(StorageError::Database(_))
        ));
        assert!(matches!(
            base.storage
                .invite(base.famille, base.arthur, base.camille, NOW),
            Err(StorageError::Database(_))
        ));
    }
}
