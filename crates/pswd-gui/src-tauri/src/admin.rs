//! Panneau d'administration de PSWD Master, et suppression d'un coffre privé sur le serveur
//! (SPEC §11).

use pswd_app::{ServerBackup, server_backup_file_name};
use pswd_core::Secret;
use pswd_infra::{AccountInfo, KeyPurpose, PendingKey, ServerVaultInfo, VaultVersion};
use serde::Serialize;
use uuid::Uuid;

use crate::error::UiError;
use crate::server::server_error;
use crate::sharing::Member;
use crate::state::{AppState, confirm};

/// Compte de l'appareil sur le serveur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStatus {
    /// Adresse locale du serveur, donnée avec les clés de connexion.
    address: String,
    account_name: String,
    /// Le panneau d'administration est proposé.
    is_admin: bool,
    public_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Device {
    id: Uuid,
    name: String,
    created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAccount {
    id: Uuid,
    name: String,
    is_admin: bool,
    devices: Vec<Device>,
}

/// Clé de connexion en attente ; `purpose` : `admin`, `member` ou `device`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    id: Uuid,
    purpose: &'static str,
    /// Compte à créer (`member`), ou compte de l'appareil (`device`).
    name: Option<String>,
    expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdminVault {
    id: Uuid,
    name: String,
    shared: bool,
    /// `None` tant que le coffre n'est pas publié.
    version: Option<i64>,
    members: Vec<Member>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptVersion {
    version: i64,
    created_at: i64,
    size: u64,
}

/// Coffre créé par une restauration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Restored {
    id: Uuid,
    name: String,
}

impl From<AccountInfo> for AdminAccount {
    fn from(account: AccountInfo) -> Self {
        Self {
            id: account.id,
            name: account.name,
            is_admin: account.is_admin,
            devices: account
                .devices
                .into_iter()
                .map(|device| Device {
                    id: device.id,
                    name: device.name,
                    created_at: device.created_at.as_millis(),
                })
                .collect(),
        }
    }
}

impl From<PendingKey> for Key {
    fn from(key: PendingKey) -> Self {
        Self {
            id: key.id,
            purpose: match key.purpose {
                KeyPurpose::Admin => "admin",
                KeyPurpose::Member => "member",
                KeyPurpose::Device => "device",
            },
            name: key.name,
            expires_at: key.expires_at.as_millis(),
        }
    }
}

impl From<ServerVaultInfo> for AdminVault {
    fn from(vault: ServerVaultInfo) -> Self {
        Self {
            id: vault.id,
            name: vault.name,
            shared: vault.shared,
            version: vault.version,
            members: vault.members.into_iter().map(Member::from).collect(),
        }
    }
}

impl From<VaultVersion> for KeptVersion {
    fn from(version: VaultVersion) -> Self {
        Self {
            version: version.version,
            created_at: version.created_at.as_millis(),
            size: version.size,
        }
    }
}

impl AppState {
    /// Compte de l'appareil, son rôle, et l'adresse publique, retenue au passage.
    pub fn account_status(&self) -> Result<AccountStatus, UiError> {
        let _one_at_a_time = self.exclusive();
        let info = self
            .observe(self.vaults.server_info())
            .map_err(server_error)?;
        let address = self.vaults.server()?.map(|server| server.local_address);
        Ok(AccountStatus {
            address: address.unwrap_or_default(),
            account_name: info.account_name,
            is_admin: info.is_admin,
            public_address: info.public_address,
        })
    }

    /// Supprime le coffre privé `name` du serveur, puis de cet appareil ; le nom est retapé pour
    /// confirmer.
    pub fn delete_everywhere(&self, name: &str, confirmation: &str) -> Result<(), UiError> {
        confirm(name, confirmation)?;
        let _one_at_a_time = self.exclusive();
        self.close_without_publishing(name);
        self.vaults.delete_everywhere(name).map_err(server_error)
    }

    pub fn admin_accounts(&self) -> Result<Vec<AdminAccount>, UiError> {
        let accounts = self.vaults.admin_accounts().map_err(server_error)?;
        Ok(accounts.into_iter().map(AdminAccount::from).collect())
    }

    /// Clé de connexion d'un nouveau membre, à montrer une seule fois.
    pub fn add_member(&self, name: &str) -> Result<Secret, UiError> {
        self.vaults.add_member(name.trim()).map_err(server_error)
    }

    /// Le nom du compte est retapé pour confirmer.
    pub fn delete_account(&self, id: Uuid, name: &str, confirmation: &str) -> Result<(), UiError> {
        confirm(name, confirmation)?;
        self.vaults.delete_account(id).map_err(server_error)
    }

    pub fn set_admin(&self, id: Uuid, is_admin: bool) -> Result<(), UiError> {
        self.vaults.set_admin(id, is_admin).map_err(server_error)
    }

    pub fn revoke_device(&self, id: Uuid) -> Result<(), UiError> {
        self.vaults.revoke_device(id).map_err(server_error)
    }

    pub fn pending_keys(&self) -> Result<Vec<Key>, UiError> {
        let keys = self.vaults.pending_keys().map_err(server_error)?;
        Ok(keys.into_iter().map(Key::from).collect())
    }

    pub fn cancel_key(&self, id: Uuid) -> Result<(), UiError> {
        self.vaults.cancel_key(id).map_err(server_error)
    }

    pub fn server_vaults(&self) -> Result<Vec<AdminVault>, UiError> {
        let vaults = self.vaults.server_vaults().map_err(server_error)?;
        Ok(vaults.into_iter().map(AdminVault::from).collect())
    }

    /// Le nom du coffre est retapé pour confirmer.
    pub fn delete_server_vault(
        &self,
        id: Uuid,
        name: &str,
        confirmation: &str,
    ) -> Result<(), UiError> {
        confirm(name, confirmation)?;
        self.vaults.delete_server_vault(id).map_err(server_error)
    }

    pub fn attach(&self, vault: Uuid, account: Uuid) -> Result<(), UiError> {
        self.vaults.attach(vault, account).map_err(server_error)
    }

    pub fn remove_member(&self, vault: Uuid, account: Uuid) -> Result<(), UiError> {
        self.vaults
            .remove_member(vault, account)
            .map_err(server_error)
    }

    pub fn versions(&self, vault: Uuid) -> Result<Vec<KeptVersion>, UiError> {
        let versions = self.vaults.versions(vault).map_err(server_error)?;
        Ok(versions.into_iter().map(KeptVersion::from).collect())
    }

    pub fn restore_version(&self, vault: Uuid, version: i64) -> Result<Restored, UiError> {
        let restored = self
            .vaults
            .restore_version(vault, version)
            .map_err(server_error)?;
        Ok(Restored {
            id: restored.id,
            name: restored.name,
        })
    }

    /// Vide, l'adresse publique est effacée.
    pub fn set_public_address(&self, address: &str) -> Result<(), UiError> {
        let _one_at_a_time = self.exclusive();
        self.vaults
            .set_public_address(address)
            .map_err(server_error)
    }

    /// Nom proposé pour la sauvegarde du serveur : `pswd-server-sauvegarde_<date>.db`.
    pub fn server_backup_file_name(&self) -> String {
        server_backup_file_name((self.clock)())
    }

    /// Sauvegarde complète du serveur, à écrire dans le fichier choisi.
    pub fn backup_server(&self) -> Result<ServerBackup, UiError> {
        self.vaults.backup_server().map_err(server_error)
    }
}

#[cfg(test)]
mod tests {
    use pswd_app::Vaults;
    use pswd_core::Timestamp;
    use pswd_core::crypto::KdfParams;
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
            || Timestamp::from_millis(1_761_229_809_999),
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

    fn code(error: &UiError) -> Value {
        sent(error)["code"].clone()
    }

    /// Arthur, administrateur ; Camille, membre du même serveur.
    fn server(dir: &TempDir) -> (AppState, AppState, TestServer) {
        let server = TestServer::start();
        let arthur = device(dir, "arthur");
        arthur
            .connect(&server.address, &server.admin_key, "PC", "Arthur")
            .unwrap();
        let key = arthur.add_member(" Camille ").unwrap();
        let camille = device(dir, "camille");
        camille
            .connect(&server.address, &key, "Téléphone", "")
            .unwrap();
        (arthur, camille, server)
    }

    fn account(state: &AppState, name: &str) -> Value {
        sent(&state.admin_accounts().unwrap())
            .as_array()
            .unwrap()
            .iter()
            .find(|account| account["name"] == name)
            .cloned()
            .unwrap()
    }

    fn id(value: &Value) -> Uuid {
        value.as_str().unwrap().parse().unwrap()
    }

    #[test]
    fn the_device_learns_its_account_and_role() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, test_server) = server(&dir);
        arthur.set_public_address("maison").unwrap();
        assert_eq!(
            sent(&camille.account_status().unwrap()),
            json!({
                "address": test_server.address,
                "accountName": "Camille",
                "isAdmin": false,
                "publicAddress": "maison",
            })
        );
        assert_eq!(
            arthur.vaults.server().unwrap().unwrap().public_address,
            Some("maison:8443".to_owned())
        );
        assert_eq!(
            code(&arthur.set_public_address("a b").unwrap_err()),
            "serverError"
        );
    }

    #[test]
    fn a_private_vault_is_deleted_from_the_server_too() {
        let dir = TempDir::new().unwrap();
        let (arthur, _camille, _server) = server(&dir);
        arthur.create("perso", &secret(PASSWORD), false).unwrap();
        arthur.exchange().unwrap();
        assert_eq!(
            arthur.delete_everywhere("perso", "autre"),
            Err(UiError::code("confirmationMismatch"))
        );
        arthur.delete_everywhere("perso", " perso ").unwrap();
        assert_eq!(arthur.current(), None);
        assert_eq!(arthur.vaults.config().unwrap().vaults.len(), 0);
        assert_eq!(arthur.server_vaults(), Ok(Vec::new()));
        assert_eq!(
            arthur.delete_everywhere("perso", "perso"),
            Err(UiError::new("unknownVault", "perso"))
        );
    }

    #[test]
    fn key_purposes_are_named_for_the_interface() {
        for (purpose, named) in [
            (KeyPurpose::Admin, "admin"),
            (KeyPurpose::Member, "member"),
            (KeyPurpose::Device, "device"),
        ] {
            let key = Key::from(PendingKey {
                id: Uuid::nil(),
                purpose,
                name: Some("Arthur".to_owned()),
                expires_at: Timestamp::from_millis(42),
            });
            assert_eq!(
                sent(&key),
                json!({
                    "id": Uuid::nil(),
                    "purpose": named,
                    "name": "Arthur",
                    "expiresAt": 42,
                })
            );
        }
    }

    #[test]
    fn accounts_devices_and_keys_are_administered() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = server(&dir);
        let camille_account = account(&arthur, "Camille");
        assert_eq!(camille_account["isAdmin"], false);
        assert_eq!(camille_account["devices"][0]["name"], "Téléphone");
        assert!(camille_account["devices"][0]["createdAt"].is_i64());
        let camille_id = id(&camille_account["id"]);

        arthur.set_admin(camille_id, true).unwrap();
        assert!(camille.account_status().unwrap().is_admin);
        arthur.set_admin(camille_id, false).unwrap();

        arthur.add_member("Dominique").unwrap();
        let keys = sent(&arthur.pending_keys().unwrap());
        assert_eq!(keys[0]["purpose"], "member");
        assert_eq!(keys[0]["name"], "Dominique");
        assert!(keys[0]["expiresAt"].is_i64());
        arthur.cancel_key(id(&keys[0]["id"])).unwrap();
        assert_eq!(arthur.pending_keys(), Ok(Vec::new()));

        arthur
            .revoke_device(id(&camille_account["devices"][0]["id"]))
            .unwrap();
        assert_eq!(code(&camille.account_status().unwrap_err()), "serverError");
        assert_eq!(
            arthur.delete_account(camille_id, "Camille", "camille"),
            Err(UiError::code("confirmationMismatch"))
        );
        arthur
            .delete_account(camille_id, "Camille", "Camille")
            .unwrap();
        assert_eq!(arthur.admin_accounts().unwrap().len(), 1);
    }

    #[test]
    fn vaults_and_versions_are_administered() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = server(&dir);
        camille.create("famille", &secret(PASSWORD), true).unwrap();
        camille.lock();
        camille.exchange().unwrap();
        let vaults = sent(&arthur.server_vaults().unwrap());
        assert_eq!(vaults[0]["name"], "famille");
        assert_eq!(vaults[0]["shared"], true);
        assert_eq!(vaults[0]["version"], 1);
        assert_eq!(vaults[0]["members"][0]["name"], "Camille");
        let famille = id(&vaults[0]["id"]);

        let versions = sent(&arthur.versions(famille).unwrap());
        assert_eq!(versions[0]["version"], 1);
        assert!(versions[0]["size"].as_u64().unwrap() > 0);
        assert!(versions[0]["createdAt"].is_i64());
        let restored = sent(&arthur.restore_version(famille, 1).unwrap());
        assert_eq!(restored["name"], "famille-v1");

        let camille_id = id(&account(&arthur, "Camille")["id"]);
        let arthur_id = id(&account(&arthur, "Arthur")["id"]);
        arthur.remove_member(famille, camille_id).unwrap();
        arthur.attach(famille, arthur_id).unwrap();
        assert_eq!(
            arthur.delete_server_vault(famille, "famille", "Famille"),
            Err(UiError::code("confirmationMismatch"))
        );
        arthur
            .delete_server_vault(famille, "famille", "famille")
            .unwrap();
        assert_eq!(arthur.server_vaults().unwrap().len(), 1);
    }

    #[test]
    fn the_server_is_backed_up() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = server(&dir);
        assert_eq!(
            arthur.server_backup_file_name(),
            "pswd-server-sauvegarde_2025-10-23_14-30-09.db"
        );
        let backup = arthur.backup_server().unwrap();
        assert!(backup.content.starts_with(b"SQLite format 3\0"));
        assert_eq!(code(&camille.backup_server().err().unwrap()), "serverError");
    }

    #[test]
    fn the_panel_needs_a_connected_device() {
        let dir = TempDir::new().unwrap();
        let alone = device(&dir, "seul");
        let id = Uuid::nil();
        let not_connected = UiError::code("notConnected");
        assert_eq!(alone.account_status().err(), Some(not_connected.clone()));
        assert_eq!(alone.admin_accounts().err(), Some(not_connected.clone()));
        assert_eq!(alone.add_member("A").err(), Some(not_connected.clone()));
        assert_eq!(alone.pending_keys().err(), Some(not_connected.clone()));
        assert_eq!(alone.server_vaults().err(), Some(not_connected.clone()));
        assert_eq!(alone.versions(id).err(), Some(not_connected.clone()));
        assert_eq!(
            alone.restore_version(id, 1).err(),
            Some(not_connected.clone())
        );
        assert_eq!(
            alone.backup_server().err().as_ref().map(code),
            Some(json!("notConnected"))
        );
        for result in [
            alone.delete_everywhere("perso", "perso"),
            alone.delete_account(id, "A", "A"),
            alone.set_admin(id, true),
            alone.revoke_device(id),
            alone.cancel_key(id),
            alone.delete_server_vault(id, "perso", "perso"),
            alone.attach(id, id),
            alone.remove_member(id, id),
            alone.set_public_address(""),
        ] {
            assert_eq!(result, Err(not_connected.clone()));
        }
    }
}
