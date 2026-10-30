//! Coffres communs depuis PSWD Master : membres, invitations, départ (SPEC §11).

use pswd_infra::Account;
use serde::Serialize;
use uuid::Uuid;

use crate::error::UiError;
use crate::server::{Exchange, server_error};
use crate::state::AppState;

/// Un compte du serveur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Member {
    id: Uuid,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Members {
    members: Vec<Member>,
    invitations: Vec<SentInvitation>,
}

/// Invitation en attente, annulable par tout membre du coffre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SentInvitation {
    id: Uuid,
    account: Member,
}

/// Invitation reçue par le compte de l'appareil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Invitation {
    id: Uuid,
    vault_name: String,
    /// Nom du compte qui a invité.
    invited_by: String,
}

impl From<Account> for Member {
    fn from(account: Account) -> Self {
        Self {
            id: account.id,
            name: account.name,
        }
    }
}

impl AppState {
    /// Comptes du serveur, pour choisir qui inviter.
    pub fn accounts(&self) -> Result<Vec<Member>, UiError> {
        let accounts = self.vaults.accounts().map_err(server_error)?;
        Ok(accounts.into_iter().map(Member::from).collect())
    }

    /// Membres du coffre commun `vault` et invitations en attente.
    pub fn members(&self, vault: &str) -> Result<Members, UiError> {
        let members = self.vaults.members(vault).map_err(server_error)?;
        Ok(Members {
            members: members.members.into_iter().map(Member::from).collect(),
            invitations: members
                .invitations
                .into_iter()
                .map(|invitation| SentInvitation {
                    id: invitation.id,
                    account: invitation.account.into(),
                })
                .collect(),
        })
    }

    pub fn invite(&self, vault: &str, account: Uuid) -> Result<(), UiError> {
        self.vaults.invite(vault, account).map_err(server_error)
    }

    /// Invitations reçues, à accepter ou refuser.
    pub fn invitations(&self) -> Result<Vec<Invitation>, UiError> {
        let invitations = self.vaults.invitations().map_err(server_error)?;
        Ok(invitations
            .into_iter()
            .map(|invitation| Invitation {
                id: invitation.id,
                vault_name: invitation.vault_name,
                invited_by: invitation.invited_by,
            })
            .collect())
    }

    /// Accepte l'invitation, puis télécharge le coffre.
    pub fn accept_invitation(&self, invitation: Uuid) -> Result<Exchange, UiError> {
        let _one_at_a_time = self.exclusive();
        let downloads = self
            .vaults
            .accept_invitation(invitation)
            .map_err(server_error)?;
        Ok(downloads.into())
    }

    /// Refuse une invitation reçue, ou annule celle d'un coffre dont le compte est membre.
    pub fn delete_invitation(&self, invitation: Uuid) -> Result<(), UiError> {
        self.vaults
            .delete_invitation(invitation)
            .map_err(server_error)
    }

    /// Quitte le coffre commun `vault` ; sa copie locale est gardée, retirée.
    pub fn leave(&self, vault: &str) -> Result<(), UiError> {
        let _one_at_a_time = self.exclusive();
        self.vaults.leave(vault).map_err(server_error)
    }
}

#[cfg(test)]
mod tests {
    use pswd_app::Vaults;
    use pswd_core::crypto::KdfParams;
    use pswd_core::{Secret, Timestamp};
    use pswd_infra::Locations;
    use pswd_server::testing::TestServer;
    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;
    use crate::testing::Silent;

    const PASSWORD: &str = "Maître-2026!";

    fn device(dir: &TempDir, name: &str) -> AppState {
        let root = dir.path().join(name);
        let vaults = Vaults::new(Locations::new(&root.join("config"), &root.join("data")));
        AppState::with_settings(
            vaults,
            Box::new(Silent),
            KdfParams::new(8 * 1024, 1, 1).unwrap(),
            || Timestamp::from_millis(1_000),
            None,
            false,
            Silent,
        )
    }

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    fn sent<T: Serialize>(value: &T) -> Value {
        serde_json::to_value(value).unwrap()
    }

    /// Arthur, administrateur, a publié son coffre commun `famille` ; Camille est membre du
    /// même serveur, sans coffre.
    fn family(dir: &TempDir) -> (AppState, AppState, TestServer) {
        let server = TestServer::start();
        let arthur = device(dir, "arthur");
        arthur
            .connect(&server.address, &server.admin_key, "PC", "Arthur")
            .unwrap();
        arthur.create("famille", &secret(PASSWORD), true).unwrap();
        arthur.lock();
        arthur.exchange().unwrap();
        let key = arthur
            .vaults
            .server()
            .unwrap()
            .unwrap()
            .client()
            .unwrap()
            .add_member("Camille")
            .unwrap();
        let camille = device(dir, "camille");
        camille
            .connect(&server.address, &key, "Téléphone", "")
            .unwrap();
        (arthur, camille, server)
    }

    fn account(state: &AppState, name: &str) -> Uuid {
        state
            .accounts()
            .unwrap()
            .into_iter()
            .find(|account| account.name == name)
            .unwrap()
            .id
    }

    fn shared_state(state: &AppState, vault: &str) -> Value {
        let status = sent(&state.server_status().unwrap());
        status["vaults"]
            .as_array()
            .unwrap()
            .iter()
            .find(|status| status["name"] == vault)
            .cloned()
            .unwrap()
    }

    #[test]
    fn an_invited_account_joins_then_leaves_the_shared_vault() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = family(&dir);
        let camille_id = account(&arthur, "Camille");
        assert_eq!(
            sent(&arthur.accounts().unwrap())
                .as_array()
                .unwrap()
                .iter()
                .map(|account| account["name"].clone())
                .collect::<Vec<_>>(),
            [json!("Arthur"), json!("Camille")]
        );
        arthur.invite("famille", camille_id).unwrap();
        let members = sent(&arthur.members("famille").unwrap());
        assert_eq!(members["members"][0]["name"], "Arthur");
        assert_eq!(members["invitations"][0]["account"]["name"], "Camille");

        let invitations = sent(&camille.invitations().unwrap());
        assert_eq!(invitations[0]["vaultName"], "famille");
        assert_eq!(invitations[0]["invitedBy"], "Arthur");
        let id = camille.invitations().unwrap()[0].id;
        assert_eq!(
            sent(&camille.accept_invitation(id).unwrap()),
            json!({
                "removed": [],
                "downloaded": [{ "name": "famille", "localName": "famille" }],
                "notDownloaded": [],
                "published": [],
                "notPublished": [],
            })
        );
        assert_eq!(
            shared_state(&camille, "famille"),
            json!({ "name": "famille", "state": "published", "shared": true })
        );
        assert_eq!(camille.invitations(), Ok(Vec::new()));
        assert_eq!(arthur.members("famille").unwrap().members.len(), 2);

        camille.leave("famille").unwrap();
        assert_eq!(shared_state(&camille, "famille")["state"], "removed");
        assert_eq!(
            camille.members("famille").err(),
            Some(UiError::new("notPublished", "famille"))
        );
        assert_eq!(arthur.members("famille").unwrap().members.len(), 1);
    }

    #[test]
    fn invitations_are_declined_or_cancelled() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = family(&dir);
        let camille_id = account(&arthur, "Camille");
        arthur.invite("famille", camille_id).unwrap();
        let id = camille.invitations().unwrap()[0].id;
        camille.delete_invitation(id).unwrap();
        assert_eq!(camille.invitations(), Ok(Vec::new()));

        arthur.invite("famille", camille_id).unwrap();
        let sent_invitation = arthur.members("famille").unwrap().invitations[0].id;
        arthur.delete_invitation(sent_invitation).unwrap();
        assert_eq!(camille.invitations(), Ok(Vec::new()));
        assert_eq!(
            sent(&camille.accept_invitation(id).unwrap_err())["code"],
            "serverError"
        );
    }

    #[test]
    fn sharing_needs_a_connected_device_and_a_published_shared_vault() {
        let dir = TempDir::new().unwrap();
        let (arthur, _camille, _server) = family(&dir);
        arthur.create("perso", &secret(PASSWORD), false).unwrap();
        assert_eq!(
            arthur.invite("perso", Uuid::nil()).err(),
            Some(UiError::new("notPublished", "perso"))
        );
        assert_eq!(
            shared_state(&arthur, "perso"),
            json!({ "name": "perso", "state": "toPublish", "shared": false })
        );
        arthur.leave("famille").unwrap();
        assert_eq!(
            arthur.leave("famille").err(),
            Some(UiError::new("notPublished", "famille"))
        );

        let alone = device(&dir, "seul");
        let not_connected = Some(UiError::code("notConnected"));
        assert_eq!(alone.accounts().err(), not_connected);
        assert_eq!(alone.invitations().err(), not_connected);
        assert_eq!(alone.members("famille").err(), not_connected);
        assert_eq!(alone.invite("famille", Uuid::nil()).err(), not_connected);
        assert_eq!(alone.accept_invitation(Uuid::nil()).err(), not_connected);
        assert_eq!(alone.delete_invitation(Uuid::nil()).err(), not_connected);
        assert_eq!(alone.leave("famille").err(), not_connected);
    }
}
