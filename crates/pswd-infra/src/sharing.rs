//! Coffres communs vus du client : comptes, membres, invitations et départs.

use pswd_core::ports::PortError;
use reqwest::StatusCode;
use reqwest::blocking::{RequestBuilder, Response};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::ServerClient;
use crate::remote::{Created, parse, status_error, unknown_vault};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Account {
    pub id: Uuid,
    pub name: String,
}

/// Invitation en attente, vue par les membres du coffre.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SentInvitation {
    pub id: Uuid,
    pub account: Account,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VaultMembers {
    pub members: Vec<Account>,
    pub invitations: Vec<SentInvitation>,
}

/// Invitation reçue par le compte de l'appareil.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ReceivedInvitation {
    pub id: Uuid,
    pub vault_id: Uuid,
    pub vault_name: String,
    /// Nom du compte qui a invité.
    pub invited_by: String,
}

impl ServerClient {
    /// Comptes du serveur, triés par nom.
    ///
    /// # Errors
    ///
    /// Si le serveur est injoignable ou répond de façon inattendue.
    pub fn accounts(&self) -> Result<Vec<Account>, PortError> {
        self.get_json("/v1/accounts")
    }

    /// Membres du coffre commun `vault` et invitations en attente.
    ///
    /// # Errors
    ///
    /// Si le compte n'en est pas membre, ou si le serveur est injoignable.
    pub fn members(&self, vault: Uuid) -> Result<VaultMembers, PortError> {
        self.get_json(&format!("/v1/vaults/{vault}/members"))
    }

    /// Invitations reçues par le compte, des plus anciennes aux plus récentes.
    ///
    /// # Errors
    ///
    /// Si le serveur est injoignable ou répond de façon inattendue.
    pub fn invitations(&self) -> Result<Vec<ReceivedInvitation>, PortError> {
        self.get_json("/v1/invitations")
    }

    /// Invite le compte `account` dans le coffre commun `vault` ; renvoie l'identifiant de
    /// l'invitation.
    ///
    /// # Errors
    ///
    /// Si le coffre est privé, si le compte est inconnu, déjà membre ou déjà invité, ou si le
    /// serveur est injoignable.
    pub fn invite(&self, vault: Uuid, account: Uuid) -> Result<Uuid, PortError> {
        let body = serde_json::json!({ "account_id": account }).to_string();
        let response = self.send(&|base| {
            self.authorized(
                self.http
                    .post(format!("{base}/v1/vaults/{vault}/invitations")),
            )
            .header(CONTENT_TYPE, "application/json")
            .body(body.clone())
        })?;
        match response.status() {
            StatusCode::CREATED => Ok(parse::<Created>(response)?.id),
            StatusCode::BAD_REQUEST => Err(PortError::new(
                "invitation impossible : coffre privé ou compte inconnu",
            )),
            StatusCode::CONFLICT => Err(PortError::new("ce compte est déjà membre ou déjà invité")),
            StatusCode::NOT_FOUND => Err(unknown_vault()),
            status => Err(status_error(status)),
        }
    }

    /// Accepte l'invitation : le compte devient membre du coffre.
    ///
    /// # Errors
    ///
    /// Si l'invitation n'existe plus, ou si le serveur est injoignable.
    pub fn accept_invitation(&self, invitation: Uuid) -> Result<(), PortError> {
        let response = self.send(&|base| {
            let url = format!("{base}/v1/invitations/{invitation}/accept");
            self.authorized(self.http.post(url))
        })?;
        no_content(&response, unknown_invitation)
    }

    /// Refuse l'invitation reçue, ou annule celle d'un coffre dont le compte est membre.
    ///
    /// # Errors
    ///
    /// Si l'invitation n'existe plus, ou si le serveur est injoignable.
    pub fn delete_invitation(&self, invitation: Uuid) -> Result<(), PortError> {
        let response = self.send(&|base| {
            self.authorized(
                self.http
                    .delete(format!("{base}/v1/invitations/{invitation}")),
            )
        })?;
        no_content(&response, unknown_invitation)
    }

    /// Le compte quitte le coffre commun `vault`.
    ///
    /// # Errors
    ///
    /// Si le coffre est privé, si le compte n'en est pas membre, ou si le serveur est
    /// injoignable.
    pub fn leave(&self, vault: Uuid) -> Result<(), PortError> {
        let response = self.send(&|base| {
            self.authorized(
                self.http
                    .delete(format!("{base}/v1/vaults/{vault}/members/self")),
            )
        })?;
        match response.status() {
            StatusCode::BAD_REQUEST => Err(PortError::new("un coffre privé ne se quitte pas")),
            _ => no_content(&response, unknown_vault),
        }
    }

    pub(crate) fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T, PortError> {
        let request = |base: &str| -> RequestBuilder {
            self.authorized(self.http.get(format!("{base}{path}")))
        };
        let response = self.send(&request)?;
        match response.status() {
            StatusCode::OK => parse(response),
            StatusCode::NOT_FOUND => Err(unknown_vault()),
            status => Err(status_error(status)),
        }
    }
}

/// Réponse attendue : `204`. `not_found` explique un `404`.
fn no_content(response: &Response, not_found: fn() -> PortError) -> Result<(), PortError> {
    match response.status() {
        StatusCode::NO_CONTENT => Ok(()),
        StatusCode::NOT_FOUND => Err(not_found()),
        status => Err(status_error(status)),
    }
}

fn unknown_invitation() -> PortError {
    PortError::new("invitation introuvable : déjà acceptée, refusée ou annulée")
}

#[cfg(test)]
mod tests {
    use pswd_core::ports::RemoteStore;
    use pswd_core::{ConnectionKey, Secret};
    use pswd_server::testing::TestServer;

    use super::*;

    /// Arthur (administrateur) et Camille, sur un serveur neuf.
    fn family() -> (ServerClient, ServerClient) {
        let server = TestServer::start();
        let connect = |key: &Secret, account: Option<&str>| {
            let key = ConnectionKey::parse(key.expose()).unwrap();
            let connection =
                ServerClient::connect(vec![server.address.clone()], &key, "PC", account).unwrap();
            ServerClient::new(
                vec![server.address.clone()],
                connection.fingerprint,
                connection.token,
            )
            .unwrap()
        };
        let arthur = connect(&server.admin_key, Some("Arthur"));
        let camille = connect(&arthur.add_member("Camille").unwrap(), None);
        (arthur, camille)
    }

    fn account(client: &ServerClient, name: &str) -> Uuid {
        client
            .accounts()
            .unwrap()
            .into_iter()
            .find(|account| account.name == name)
            .unwrap()
            .id
    }

    #[test]
    fn an_invited_account_joins_then_leaves_a_shared_vault() {
        let (arthur, camille) = family();
        let vault = arthur.create_vault("famille", true).unwrap();
        let names: Vec<String> = arthur
            .accounts()
            .unwrap()
            .into_iter()
            .map(|a| a.name)
            .collect();
        assert_eq!(names, ["Arthur", "Camille"]);

        let invitation = arthur.invite(vault, account(&arthur, "Camille")).unwrap();
        let members = arthur.members(vault).unwrap();
        assert_eq!(members.members.len(), 1);
        assert_eq!(members.invitations[0].id, invitation);
        assert_eq!(members.invitations[0].account.name, "Camille");
        assert_eq!(
            camille.invitations().unwrap(),
            [ReceivedInvitation {
                id: invitation,
                vault_id: vault,
                vault_name: "famille".to_owned(),
                invited_by: "Arthur".to_owned(),
            }]
        );

        camille.accept_invitation(invitation).unwrap();
        assert!(camille.list_vaults().unwrap()[0].shared);
        assert_eq!(camille.members(vault).unwrap().members.len(), 2);

        camille.leave(vault).unwrap();
        let gone = camille.vault(vault).fetch().unwrap_err();
        assert!(gone.is_removed(), "{gone}");
        assert!(camille.members(vault).unwrap_err().is_removed());
        assert_eq!(camille.list_vaults().unwrap(), []);
    }

    #[test]
    fn invitations_are_refused_or_cancelled() {
        let (arthur, camille) = family();
        let vault = arthur.create_vault("famille", true).unwrap();
        let camille_id = account(&arthur, "Camille");
        let invitation = arthur.invite(vault, camille_id).unwrap();
        camille.delete_invitation(invitation).unwrap();
        let unknown = Err(PortError::new(
            "invitation introuvable : déjà acceptée, refusée ou annulée",
        ));
        assert_eq!(arthur.delete_invitation(invitation), unknown);
        assert_eq!(camille.accept_invitation(invitation), unknown);

        let invitation = arthur.invite(vault, camille_id).unwrap();
        arthur.delete_invitation(invitation).unwrap();
        assert_eq!(camille.invitations().unwrap(), []);
    }

    #[test]
    fn refusals_are_explained() {
        let (arthur, camille) = family();
        let shared = arthur.create_vault("famille", true).unwrap();
        let private = arthur.create_vault("perso", false).unwrap();
        let arthur_id = account(&arthur, "Arthur");
        let camille_id = account(&arthur, "Camille");
        let impossible = Err(PortError::new(
            "invitation impossible : coffre privé ou compte inconnu",
        ));
        assert_eq!(arthur.invite(private, camille_id), impossible);
        assert_eq!(arthur.invite(shared, Uuid::new_v4()), impossible);
        assert_eq!(
            arthur.invite(shared, arthur_id),
            Err(PortError::new("ce compte est déjà membre ou déjà invité"))
        );
        let unknown = PortError::new("coffre inconnu du serveur");
        assert_eq!(camille.invite(shared, arthur_id), Err(unknown.clone()));
        assert_eq!(camille.members(shared), Err(unknown.clone()));
        assert_eq!(camille.leave(shared), Err(unknown));
        assert_eq!(
            arthur.leave(private),
            Err(PortError::new("un coffre privé ne se quitte pas"))
        );
    }

    #[test]
    fn a_disconnected_device_is_refused() {
        let (arthur, _) = family();
        let vault = arthur.create_vault("famille", true).unwrap();
        arthur.disconnect().unwrap();
        let refused = PortError::new("appareil non reconnu par le serveur : déconnecté ou révoqué");
        assert_eq!(arthur.accounts(), Err(refused.clone()));
        assert_eq!(arthur.invite(vault, Uuid::new_v4()), Err(refused.clone()));
        assert_eq!(
            arthur.accept_invitation(Uuid::new_v4()),
            Err(refused.clone())
        );
        assert_eq!(arthur.leave(vault), Err(refused));
    }

    #[test]
    fn an_offline_server_is_reported() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for connection in listener.incoming() {
                drop(connection);
            }
        });
        let fingerprint = [0; pswd_core::crypto::FINGERPRINT_LEN];
        let token = Secret::new("jeton".to_owned());
        let offline = ServerClient::new(vec![address], fingerprint, token).unwrap();
        let unreachable = PortError::unreachable("serveur injoignable");
        let id = Uuid::new_v4();
        assert_eq!(offline.invitations().err(), Some(unreachable.clone()));
        assert_eq!(offline.invite(id, id).err(), Some(unreachable.clone()));
        assert_eq!(
            offline.accept_invitation(id).err(),
            Some(unreachable.clone())
        );
        assert_eq!(
            offline.delete_invitation(id).err(),
            Some(unreachable.clone())
        );
        assert_eq!(offline.leave(id).err(), Some(unreachable));
    }
}
