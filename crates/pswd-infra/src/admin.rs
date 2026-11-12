//! Panneau d'administration vu du client, et ce que tout appareil apprend du serveur.

use pswd_core::Timestamp;
use pswd_core::ports::PortError;
use reqwest::blocking::Response;
use reqwest::header::CONTENT_TYPE;
use reqwest::{Method, StatusCode};
use serde::Deserialize;
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::remote::{body, parse, status_error};
use crate::{Account, ServerClient};

const UNKNOWN_VAULT: &str = "coffre inconnu du serveur";
const UNKNOWN_ACCOUNT: &str = "compte inconnu du serveur";
const LAST_ADMIN: &str = "le dernier administrateur garde son rôle et son compte";
/// Même refus (`409`) pour le dernier administrateur et pour un autre administrateur.
const ADMIN_ACCOUNT: &str =
    "le dernier administrateur garde son compte, et un autre administrateur perd d'abord son rôle";

/// Compte de l'appareil, et adresse publique du serveur.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ServerInfo {
    pub account_id: Uuid,
    pub account_name: String,
    pub is_admin: bool,
    pub public_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DeviceInfo {
    pub id: Uuid,
    pub name: String,
    pub created_at: Timestamp,
}

/// Compte du serveur et ses appareils.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AccountInfo {
    pub id: Uuid,
    pub name: String,
    pub is_admin: bool,
    pub devices: Vec<DeviceInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyPurpose {
    Admin,
    /// Un nouveau compte, nommé par la clé.
    Member,
    /// Un appareil de plus pour le compte nommé par la clé.
    Device,
}

/// Clé de connexion pas encore utilisée.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct PendingKey {
    pub id: Uuid,
    pub purpose: KeyPurpose,
    pub name: Option<String>,
    pub expires_at: Timestamp,
}

/// Coffre du serveur, vu par un administrateur ; `version` vaut `None` tant qu'il n'est pas
/// publié.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ServerVaultInfo {
    pub id: Uuid,
    pub name: String,
    pub shared: bool,
    pub version: Option<i64>,
    pub members: Vec<Account>,
}

/// Version conservée d'un coffre.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct VaultVersion {
    pub version: i64,
    pub created_at: Timestamp,
    /// Taille du fichier, en octets.
    pub size: u64,
}

/// Coffre créé par la restauration d'une version.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RestoredVault {
    pub id: Uuid,
    pub name: String,
}

impl ServerClient {
    /// Compte de l'appareil, son rôle, et l'adresse publique du serveur.
    ///
    /// # Errors
    ///
    /// Si le serveur est injoignable ou répond de façon inattendue.
    pub fn server_info(&self) -> Result<ServerInfo, PortError> {
        self.get_json("/v1/server")
    }

    /// Supprime partout le coffre privé `vault` du compte, ou le coffre commun dont il est le
    /// dernier membre : les appareils de ses membres en suppriment leur copie.
    ///
    /// # Errors
    ///
    /// Si le coffre est commun et a d'autres membres, s'il est inconnu ou déjà retiré, ou si le
    /// serveur est injoignable.
    pub fn delete_own_vault(&self, vault: Uuid) -> Result<(), PortError> {
        let response = self.request(&Method::DELETE, &format!("/v1/vaults/{vault}"), None)?;
        done(
            &response,
            &[
                (
                    StatusCode::BAD_REQUEST,
                    "un coffre commun qui a d'autres membres se quitte : il ne se supprime pas du serveur",
                ),
                (StatusCode::NOT_FOUND, UNKNOWN_VAULT),
            ],
        )
    }

    /// Comptes du serveur et leurs appareils ; réservé aux administrateurs.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas administrateur, ou si le serveur est injoignable.
    pub fn admin_accounts(&self) -> Result<Vec<AccountInfo>, PortError> {
        self.get_json("/v1/admin/accounts")
    }

    /// Supprime le compte, ses appareils et ses clés ; ses coffres restent sur le serveur.
    ///
    /// # Errors
    ///
    /// Si le compte est inconnu, est le dernier administrateur ou un autre administrateur que
    /// celui de l'appareil, ou si le serveur est injoignable.
    pub fn delete_account(&self, account: Uuid) -> Result<(), PortError> {
        let uri = format!("/v1/admin/accounts/{account}");
        let response = self.request(&Method::DELETE, &uri, None)?;
        done(
            &response,
            &[
                (StatusCode::CONFLICT, ADMIN_ACCOUNT),
                (StatusCode::NOT_FOUND, UNKNOWN_ACCOUNT),
            ],
        )
    }

    /// Donne ou retire le rôle d'administrateur.
    ///
    /// # Errors
    ///
    /// Si le compte est inconnu, s'il s'agit de retirer son rôle au dernier administrateur, ou si
    /// le serveur est injoignable.
    pub fn set_admin(&self, account: Uuid, is_admin: bool) -> Result<(), PortError> {
        let uri = format!("/v1/admin/accounts/{account}/admin");
        let body = serde_json::json!({ "is_admin": is_admin });
        let response = self.request(&Method::PUT, &uri, Some(body))?;
        done(
            &response,
            &[
                (StatusCode::CONFLICT, LAST_ADMIN),
                (StatusCode::NOT_FOUND, UNKNOWN_ACCOUNT),
            ],
        )
    }

    /// Révoque un appareil : son jeton ne vaut plus rien.
    ///
    /// # Errors
    ///
    /// Si l'appareil est inconnu, ou si le serveur est injoignable.
    pub fn revoke_device(&self, device: Uuid) -> Result<(), PortError> {
        let response = self.request(
            &Method::DELETE,
            &format!("/v1/admin/devices/{device}"),
            None,
        )?;
        done(
            &response,
            &[(StatusCode::NOT_FOUND, "appareil inconnu du serveur")],
        )
    }

    /// Clés de connexion pas encore utilisées, de la plus proche de son expiration à la plus
    /// lointaine.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas administrateur, ou si le serveur est injoignable.
    pub fn pending_keys(&self) -> Result<Vec<PendingKey>, PortError> {
        self.get_json("/v1/admin/keys")
    }

    /// # Errors
    ///
    /// Si la clé a déjà servi, a expiré ou a été annulée, ou si le serveur est injoignable.
    pub fn cancel_key(&self, key: Uuid) -> Result<(), PortError> {
        let response = self.request(&Method::DELETE, &format!("/v1/admin/keys/{key}"), None)?;
        done(
            &response,
            &[(
                StatusCode::NOT_FOUND,
                "clé introuvable : déjà utilisée, expirée ou annulée",
            )],
        )
    }

    /// Coffres du serveur, de tous les comptes, avec leurs membres.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas administrateur, ou si le serveur est injoignable.
    pub fn server_vaults(&self) -> Result<Vec<ServerVaultInfo>, PortError> {
        self.get_json("/v1/admin/vaults")
    }

    /// Supprime le coffre du serveur : son contenu et ses versions sont effacés.
    ///
    /// # Errors
    ///
    /// Si le coffre est inconnu, ou si le serveur est injoignable.
    pub fn delete_server_vault(&self, vault: Uuid) -> Result<(), PortError> {
        let response = self.request(&Method::DELETE, &format!("/v1/admin/vaults/{vault}"), None)?;
        done(&response, &[(StatusCode::NOT_FOUND, UNKNOWN_VAULT)])
    }

    /// Rattache le compte `account` au coffre `vault`, qui n'a plus de membre.
    ///
    /// # Errors
    ///
    /// Si le coffre ou le compte sont inconnus, si le coffre a encore des membres, ou si le
    /// serveur est injoignable.
    pub fn attach(&self, vault: Uuid, account: Uuid) -> Result<(), PortError> {
        let uri = format!("/v1/admin/vaults/{vault}/members");
        let body = serde_json::json!({ "account_id": account });
        let response = self.request(&Method::POST, &uri, Some(body))?;
        done(
            &response,
            &[
                (StatusCode::BAD_REQUEST, UNKNOWN_ACCOUNT),
                (StatusCode::CONFLICT, "le coffre a encore des membres"),
                (StatusCode::NOT_FOUND, UNKNOWN_VAULT),
            ],
        )
    }

    /// Retire le compte `account` du coffre `vault`.
    ///
    /// # Errors
    ///
    /// Si le coffre est inconnu ou si le compte n'en est pas membre, ou si le serveur est
    /// injoignable.
    pub fn remove_member(&self, vault: Uuid, account: Uuid) -> Result<(), PortError> {
        let uri = format!("/v1/admin/vaults/{vault}/members/{account}");
        let response = self.request(&Method::DELETE, &uri, None)?;
        done(
            &response,
            &[(
                StatusCode::NOT_FOUND,
                "coffre inconnu, ou compte qui n'en est pas membre",
            )],
        )
    }

    /// Versions conservées du coffre, de la plus récente à la plus ancienne.
    ///
    /// # Errors
    ///
    /// Si le coffre est inconnu, ou si le serveur est injoignable.
    pub fn versions(&self, vault: Uuid) -> Result<Vec<VaultVersion>, PortError> {
        self.get_json(&format!("/v1/admin/vaults/{vault}/versions"))
    }

    /// Crée un nouveau coffre à partir de la version `version` du coffre `vault`.
    ///
    /// # Errors
    ///
    /// Si le coffre ou la version n'existent pas, ou si le serveur est injoignable.
    pub fn restore_version(&self, vault: Uuid, version: i64) -> Result<RestoredVault, PortError> {
        let uri = format!("/v1/admin/vaults/{vault}/versions/{version}/restore");
        let response = self.request(&Method::POST, &uri, None)?;
        match response.status() {
            StatusCode::CREATED => parse(response),
            StatusCode::NOT_FOUND => Err(PortError::new("version introuvable sur le serveur")),
            status => Err(status_error(status)),
        }
    }

    /// Renseigne l'adresse publique du serveur ; `None` l'efface.
    ///
    /// # Errors
    ///
    /// Si l'adresse est invalide, ou si le serveur est injoignable.
    pub fn set_public_address(&self, address: Option<&str>) -> Result<(), PortError> {
        let body = serde_json::json!({ "public_address": address });
        let response = self.request(&Method::PUT, "/v1/admin/settings", Some(body))?;
        done(
            &response,
            &[(StatusCode::BAD_REQUEST, "adresse publique invalide")],
        )
    }

    /// Sauvegarde complète du serveur : une base SQLite, qui contient la clé privée de son
    /// certificat.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas administrateur, ou si le serveur est injoignable.
    pub fn backup(&self) -> Result<Zeroizing<Vec<u8>>, PortError> {
        let response = self.request(&Method::GET, "/v1/admin/backup", None)?;
        match response.status() {
            StatusCode::OK => Ok(Zeroizing::new(body(response)?)),
            status => Err(status_error(status)),
        }
    }

    fn request(
        &self,
        method: &Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<Response, PortError> {
        let body = body.map(|json| json.to_string());
        self.send(&|base| {
            let request =
                self.authorized(self.http.request(method.clone(), format!("{base}{path}")));
            match &body {
                Some(json) => request
                    .header(CONTENT_TYPE, "application/json")
                    .body(json.clone()),
                None => request,
            }
        })
    }
}

/// Réponse attendue : `204`. `refusals` explique les autres réponses prévues.
fn done(response: &Response, refusals: &[(StatusCode, &str)]) -> Result<(), PortError> {
    let status = response.status();
    if status == StatusCode::NO_CONTENT {
        return Ok(());
    }
    match refusals.iter().find(|(code, _)| *code == status) {
        Some((_, message)) => Err(PortError::new(*message)),
        None => Err(status_error(status)),
    }
}

#[cfg(test)]
mod tests {
    use pswd_core::crypto::KdfParams;
    use pswd_core::ports::RemoteStore;
    use pswd_core::{ConnectionKey, Secret, Vault, VaultFile};
    use pswd_server::testing::TestServer;

    use super::*;

    /// Arthur (administrateur) et Camille, sur un serveur neuf.
    fn server() -> (ServerClient, ServerClient) {
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

    fn vault_file() -> Vec<u8> {
        let params = KdfParams::new(8 * 1024, 1, 1).unwrap();
        let password = Secret::new("maître".to_owned());
        let (file, _) =
            VaultFile::create(&password, params, Timestamp::from_millis(1_000)).unwrap();
        file.encrypt(&Vault::new()).unwrap()
    }

    /// Coffre créé et publié par `client`.
    fn published(client: &ServerClient, name: &str, shared: bool) -> Uuid {
        let id = client.create_vault(name, shared).unwrap();
        client.vault(id).push(&vault_file(), None).unwrap();
        id
    }

    fn account(client: &ServerClient, name: &str) -> AccountInfo {
        client
            .admin_accounts()
            .unwrap()
            .into_iter()
            .find(|account| account.name == name)
            .unwrap()
    }

    fn refused(message: &str) -> PortError {
        PortError::new(message)
    }

    #[test]
    fn devices_learn_their_account_and_the_public_address() {
        let (arthur, camille) = server();
        let info = camille.server_info().unwrap();
        assert_eq!(
            (
                info.account_name.as_str(),
                info.is_admin,
                info.public_address
            ),
            ("Camille", false, None)
        );
        arthur.set_public_address(Some("maison:8443")).unwrap();
        assert_eq!(
            camille.server_info().unwrap().public_address.as_deref(),
            Some("maison:8443")
        );
        assert_eq!(
            arthur.set_public_address(Some("https://maison")),
            Err(refused("adresse publique invalide"))
        );
        arthur.set_public_address(None).unwrap();
        assert_eq!(camille.server_info().unwrap().public_address, None);
    }

    #[test]
    fn an_account_deletes_its_own_vaults() {
        let (arthur, camille) = server();
        let private = published(&arthur, "perso", false);
        let shared = published(&arthur, "famille", true);
        let invitation = arthur
            .invite(shared, account(&arthur, "Camille").id)
            .unwrap();
        camille.accept_invitation(invitation).unwrap();
        assert_eq!(
            arthur.delete_own_vault(shared),
            Err(refused(
                "un coffre commun qui a d'autres membres se quitte : il ne se supprime pas du serveur"
            ))
        );
        camille.leave(shared).unwrap();
        arthur.delete_own_vault(shared).unwrap();
        arthur.delete_own_vault(private).unwrap();
        assert!(arthur.delete_own_vault(private).unwrap_err().is_removed());
        assert_eq!(
            arthur.delete_own_vault(Uuid::new_v4()),
            Err(refused(UNKNOWN_VAULT))
        );
    }

    #[test]
    fn accounts_devices_and_roles_are_managed() {
        let (arthur, camille) = server();
        let accounts = arthur.admin_accounts().unwrap();
        assert_eq!(accounts.len(), 2);
        assert!(accounts[0].is_admin);
        assert_eq!(accounts[1].devices[0].name, "PC");
        let arthur_id = accounts[0].id;
        let camille_id = account(&arthur, "Camille").id;

        assert_eq!(arthur.set_admin(arthur_id, false), Err(refused(LAST_ADMIN)));
        assert_eq!(
            arthur.delete_account(arthur_id),
            Err(refused(ADMIN_ACCOUNT))
        );
        arthur.set_admin(camille_id, true).unwrap();
        assert!(camille.admin_accounts().is_ok());
        assert_eq!(
            arthur.delete_account(camille_id),
            Err(refused(ADMIN_ACCOUNT))
        );
        arthur.set_admin(camille_id, false).unwrap();
        let unknown = Uuid::new_v4();
        assert_eq!(
            arthur.set_admin(unknown, true),
            Err(refused(UNKNOWN_ACCOUNT))
        );

        let device = account(&arthur, "Camille").devices[0].id;
        arthur.revoke_device(device).unwrap();
        assert_eq!(
            arthur.revoke_device(device),
            Err(refused("appareil inconnu du serveur"))
        );
        arthur.delete_account(camille_id).unwrap();
        assert_eq!(
            arthur.delete_account(camille_id),
            Err(refused(UNKNOWN_ACCOUNT))
        );
    }

    #[test]
    fn pending_keys_are_listed_and_cancelled() {
        let (arthur, camille) = server();
        arthur.add_member("Dominique").unwrap();
        camille.new_device_key().unwrap();
        let keys = arthur.pending_keys().unwrap();
        let purposes: Vec<(KeyPurpose, Option<&str>)> = keys
            .iter()
            .map(|key| (key.purpose, key.name.as_deref()))
            .collect();
        assert!(purposes.contains(&(KeyPurpose::Member, Some("Dominique"))));
        assert!(purposes.contains(&(KeyPurpose::Device, Some("Camille"))));
        arthur.cancel_key(keys[0].id).unwrap();
        assert_eq!(
            arthur.cancel_key(keys[0].id),
            Err(refused(
                "clé introuvable : déjà utilisée, expirée ou annulée"
            ))
        );
        assert_eq!(arthur.pending_keys().unwrap().len(), 1);
    }

    #[test]
    fn vaults_are_managed_by_administrators() {
        let (arthur, camille) = server();
        let famille = published(&camille, "famille", true);
        let vaults = arthur.server_vaults().unwrap();
        assert_eq!(vaults[0].name, "famille");
        assert_eq!(vaults[0].version, Some(1));
        assert_eq!(vaults[0].members[0].name, "Camille");

        let versions = arthur.versions(famille).unwrap();
        assert_eq!(versions[0].version, 1);
        assert_eq!(versions[0].size, vault_file().len() as u64);
        let restored = arthur.restore_version(famille, 1).unwrap();
        assert_eq!(restored.name, "famille-v1");
        assert_eq!(
            arthur.restore_version(famille, 7),
            Err(refused("version introuvable sur le serveur"))
        );
        assert_eq!(arthur.versions(Uuid::new_v4()), Err(refused(UNKNOWN_VAULT)));

        let arthur_id = account(&arthur, "Arthur").id;
        let camille_id = account(&arthur, "Camille").id;
        assert_eq!(
            arthur.attach(famille, arthur_id),
            Err(refused("le coffre a encore des membres"))
        );
        arthur.remove_member(famille, camille_id).unwrap();
        assert_eq!(
            arthur.remove_member(famille, camille_id),
            Err(refused("coffre inconnu, ou compte qui n'en est pas membre"))
        );
        assert!(camille.vault(famille).fetch().unwrap_err().is_removed());
        assert_eq!(
            arthur.attach(famille, Uuid::new_v4()),
            Err(refused(UNKNOWN_ACCOUNT))
        );
        assert_eq!(
            arthur.attach(Uuid::new_v4(), arthur_id),
            Err(refused(UNKNOWN_VAULT))
        );
        arthur.attach(famille, arthur_id).unwrap();
        assert!(arthur.vault(famille).fetch().unwrap().is_some());

        arthur.delete_server_vault(famille).unwrap();
        assert_eq!(
            arthur.delete_server_vault(famille),
            Err(refused(UNKNOWN_VAULT))
        );
    }

    #[test]
    fn backup_is_a_sqlite_database() {
        let (arthur, camille) = server();
        assert!(arthur.backup().unwrap().starts_with(b"SQLite format 3\0"));
        assert_eq!(
            camille.backup(),
            Err(refused("action réservée aux administrateurs"))
        );
    }

    #[test]
    fn members_cannot_administer() {
        let (_, camille) = server();
        let id = Uuid::new_v4();
        let forbidden = Err::<(), _>(refused("action réservée aux administrateurs"));
        assert_eq!(camille.admin_accounts().map(drop), forbidden);
        assert_eq!(camille.pending_keys().map(drop), forbidden);
        assert_eq!(camille.server_vaults().map(drop), forbidden);
        assert_eq!(camille.restore_version(id, 1).map(drop), forbidden);
        for result in [
            camille.delete_account(id),
            camille.set_admin(id, true),
            camille.revoke_device(id),
            camille.cancel_key(id),
            camille.delete_server_vault(id),
            camille.attach(id, id),
            camille.remove_member(id, id),
            camille.set_public_address(None),
        ] {
            assert_eq!(result, forbidden);
        }
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
        let unreachable = Err(PortError::unreachable("serveur injoignable"));
        let id = Uuid::new_v4();
        assert_eq!(offline.backup().map(drop), unreachable);
        assert_eq!(offline.restore_version(id, 1).map(drop), unreachable);
        assert_eq!(offline.delete_own_vault(id), unreachable);
        assert_eq!(offline.attach(id, id), unreachable);
    }
}
