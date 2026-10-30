//! Ce que l'appareil apprend du serveur, suppression d'un coffre sur le serveur, et panneau
//! d'administration (SPEC §11).

use pswd_core::Secret;
use pswd_infra::{
    AccountInfo, PendingKey, RestoredVault, ServerConfig, ServerInfo, ServerVaultInfo, VaultVersion,
};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::server::with_default_port;
use crate::{AppError, Vaults, now, server_backup_file_name};

/// Sauvegarde du serveur, à enregistrer sous le nom proposé. Elle contient la clé privée du
/// certificat du serveur.
pub struct ServerBackup {
    pub file_name: String,
    pub content: Zeroizing<Vec<u8>>,
}

impl Vaults {
    /// Compte de l'appareil et son rôle ; l'adresse publique du serveur est retenue au passage.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté, si le serveur est injoignable, ou si `server.toml` ne
    /// peut pas être écrit.
    pub fn server_info(&self) -> Result<ServerInfo, AppError> {
        let (client, server) = self.client()?;
        let info = client.server_info()?;
        self.remember_public_address(&server, info.public_address.as_deref())?;
        Ok(info)
    }

    /// Retient dans `server.toml` l'adresse publique donnée par le serveur, si elle a changé.
    pub(crate) fn remember_public_address(
        &self,
        server: &ServerConfig,
        address: Option<&str>,
    ) -> Result<(), AppError> {
        let address = address.map(with_default_port);
        if server.public_address == address {
            return Ok(());
        }
        let updated = ServerConfig {
            local_address: server.local_address.clone(),
            public_address: address,
            fingerprint: server.fingerprint,
            token: server.token.clone(),
        };
        self.server_file().save(&updated)?;
        Ok(())
    }

    /// Supprime le coffre privé `name` du serveur, puis de cet appareil. Un coffre déjà supprimé
    /// du serveur depuis un autre appareil est seulement supprimé ici.
    ///
    /// # Errors
    ///
    /// Si le coffre n'est pas publié ou est commun, ou si le serveur est injoignable : rien n'est
    /// alors supprimé.
    pub fn delete_everywhere(&self, name: &str) -> Result<(), AppError> {
        let (client, server) = self.client()?;
        match client.delete_own_vault(self.published_id(&server, name)?) {
            Err(error) if !error.is_removed() => return Err(error.into()),
            _ => {}
        }
        self.delete(name)
    }

    /// Comptes du serveur et leurs appareils.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté ou administrateur, ou si le serveur est injoignable.
    pub fn admin_accounts(&self) -> Result<Vec<AccountInfo>, AppError> {
        Ok(self.client()?.0.admin_accounts()?)
    }

    /// Clé de connexion d'un nouveau membre `name`, à lui transmettre avec l'adresse du serveur.
    ///
    /// # Errors
    ///
    /// Si le nom est invalide, si l'appareil n'est pas administrateur, ou si le serveur est
    /// injoignable.
    pub fn add_member(&self, name: &str) -> Result<Secret, AppError> {
        Ok(self.client()?.0.add_member(name)?)
    }

    /// # Errors
    ///
    /// Si le compte est inconnu ou est le dernier administrateur, ou si le serveur est
    /// injoignable.
    pub fn delete_account(&self, account: Uuid) -> Result<(), AppError> {
        Ok(self.client()?.0.delete_account(account)?)
    }

    /// # Errors
    ///
    /// Si le compte est inconnu, s'il s'agit de retirer son rôle au dernier administrateur, ou si
    /// le serveur est injoignable.
    pub fn set_admin(&self, account: Uuid, is_admin: bool) -> Result<(), AppError> {
        Ok(self.client()?.0.set_admin(account, is_admin)?)
    }

    /// # Errors
    ///
    /// Si l'appareil est inconnu, ou si le serveur est injoignable.
    pub fn revoke_device(&self, device: Uuid) -> Result<(), AppError> {
        Ok(self.client()?.0.revoke_device(device)?)
    }

    /// # Errors
    ///
    /// Si l'appareil n'est pas administrateur, ou si le serveur est injoignable.
    pub fn pending_keys(&self) -> Result<Vec<PendingKey>, AppError> {
        Ok(self.client()?.0.pending_keys()?)
    }

    /// # Errors
    ///
    /// Si la clé a déjà servi, a expiré ou a été annulée, ou si le serveur est injoignable.
    pub fn cancel_key(&self, key: Uuid) -> Result<(), AppError> {
        Ok(self.client()?.0.cancel_key(key)?)
    }

    /// Coffres du serveur, de tous les comptes.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas administrateur, ou si le serveur est injoignable.
    pub fn server_vaults(&self) -> Result<Vec<ServerVaultInfo>, AppError> {
        Ok(self.client()?.0.server_vaults()?)
    }

    /// Supprime un coffre du serveur ; les copies locales des appareils sont gardées, retirées.
    ///
    /// # Errors
    ///
    /// Si le coffre est inconnu, ou si le serveur est injoignable.
    pub fn delete_server_vault(&self, vault: Uuid) -> Result<(), AppError> {
        Ok(self.client()?.0.delete_server_vault(vault)?)
    }

    /// Rattache un compte à un coffre qui n'a plus de membre.
    ///
    /// # Errors
    ///
    /// Si le coffre ou le compte sont inconnus, si le coffre a encore des membres, ou si le
    /// serveur est injoignable.
    pub fn attach(&self, vault: Uuid, account: Uuid) -> Result<(), AppError> {
        Ok(self.client()?.0.attach(vault, account)?)
    }

    /// # Errors
    ///
    /// Si le coffre est inconnu ou si le compte n'en est pas membre, ou si le serveur est
    /// injoignable.
    pub fn remove_member(&self, vault: Uuid, account: Uuid) -> Result<(), AppError> {
        Ok(self.client()?.0.remove_member(vault, account)?)
    }

    /// Versions conservées d'un coffre, de la plus récente à la plus ancienne.
    ///
    /// # Errors
    ///
    /// Si le coffre est inconnu, ou si le serveur est injoignable.
    pub fn versions(&self, vault: Uuid) -> Result<Vec<VaultVersion>, AppError> {
        Ok(self.client()?.0.versions(vault)?)
    }

    /// Crée un nouveau coffre à partir d'une version ; ses membres le reçoivent au prochain
    /// échange.
    ///
    /// # Errors
    ///
    /// Si le coffre ou la version n'existent pas, ou si le serveur est injoignable.
    pub fn restore_version(&self, vault: Uuid, version: i64) -> Result<RestoredVault, AppError> {
        Ok(self.client()?.0.restore_version(vault, version)?)
    }

    /// Renseigne l'adresse publique du serveur (`hôte` ou `hôte:port`) ; vide, elle est effacée.
    /// Cet appareil la retient aussitôt, les autres au prochain échange.
    ///
    /// # Errors
    ///
    /// Si l'adresse est invalide, si l'appareil n'est pas administrateur, ou si le serveur est
    /// injoignable.
    pub fn set_public_address(&self, address: &str) -> Result<(), AppError> {
        let address = Some(address.trim()).filter(|address| !address.is_empty());
        let (client, server) = self.client()?;
        client.set_public_address(address)?;
        self.remember_public_address(&server, address)
    }

    /// Sauvegarde complète du serveur, et le nom de fichier proposé.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas administrateur, ou si le serveur est injoignable.
    pub fn backup_server(&self) -> Result<ServerBackup, AppError> {
        let content = self.client()?.0.backup()?;
        Ok(ServerBackup {
            file_name: server_backup_file_name(now()),
            content,
        })
    }
}

#[cfg(test)]
mod tests {
    use pswd_core::crypto::KdfParams;
    use pswd_core::ports::PortError;
    use pswd_core::{ConnectionKey, Timestamp};
    use pswd_infra::Locations;
    use pswd_server::testing::TestServer;
    use tempfile::TempDir;

    use super::*;
    use crate::VaultState;

    fn device(dir: &TempDir, name: &str) -> Vaults {
        let root = dir.path().join(name);
        Vaults::new(Locations::new(&root.join("config"), &root.join("data")))
    }

    fn key(text: &Secret) -> ConnectionKey {
        ConnectionKey::parse(text.expose()).unwrap()
    }

    fn create(vaults: &Vaults, name: &str, shared: bool) {
        let params = KdfParams::new(8 * 1024, 1, 1).unwrap();
        let at = Timestamp::from_millis(0);
        let password = Secret::new("Maître-2026!".to_owned());
        if shared {
            vaults.create_shared(name, &password, params, at).unwrap();
        } else {
            vaults.create(name, &password, params, at).unwrap();
        }
    }

    /// Connecte `vaults` comme administrateur d'un serveur neuf.
    fn connect_admin(vaults: &Vaults) -> TestServer {
        let server = TestServer::start();
        vaults
            .connect(
                &server.address,
                &key(&server.admin_key),
                "PC",
                Some("Arthur"),
            )
            .unwrap();
        server
    }

    /// Connecte `other` à `server` avec `key`.
    fn connect(other: &Vaults, server: &TestServer, key_text: &Secret) {
        other
            .connect(&server.address, &key(key_text), "Portable", None)
            .unwrap();
    }

    fn public_address(vaults: &Vaults) -> Option<String> {
        vaults.server().unwrap().unwrap().public_address
    }

    fn account_id(vaults: &Vaults, name: &str) -> Uuid {
        vaults
            .admin_accounts()
            .unwrap()
            .into_iter()
            .find(|account| account.name == name)
            .unwrap()
            .id
    }

    #[test]
    fn the_public_address_reaches_every_device() {
        let dir = TempDir::new().unwrap();
        let (a, b) = (device(&dir, "a"), device(&dir, "b"));
        let server = connect_admin(&a);
        connect(&b, &server, &a.new_device_key().unwrap());

        a.set_public_address(" maison.example.org ").unwrap();
        assert_eq!(
            public_address(&a).as_deref(),
            Some("maison.example.org:8443")
        );
        assert_eq!(public_address(&b), None);
        b.download_vaults().unwrap();
        assert_eq!(
            public_address(&b).as_deref(),
            Some("maison.example.org:8443")
        );
        let info = b.server_info().unwrap();
        assert_eq!(
            (info.account_name.as_str(), info.is_admin),
            ("Arthur", true)
        );

        a.set_public_address("").unwrap();
        assert_eq!(public_address(&a), None);
        b.server_info().unwrap();
        assert_eq!(public_address(&b), None);
        assert_eq!(
            a.set_public_address("https://maison"),
            Err(PortError::new("adresse publique invalide").into())
        );
    }

    #[test]
    fn a_private_vault_is_deleted_everywhere() {
        let dir = TempDir::new().unwrap();
        let (a, b) = (device(&dir, "a"), device(&dir, "b"));
        create(&a, "perso", false);
        create(&a, "famille", true);
        create(&a, "brouillon", false);
        let server = connect_admin(&a);
        a.publish_vaults().unwrap();
        connect(&b, &server, &a.new_device_key().unwrap());
        b.download_vaults().unwrap();
        create(&a, "neuf", false);

        assert_eq!(
            a.delete_everywhere("famille"),
            Err(
                PortError::new("un coffre commun se quitte : il ne se supprime pas du serveur")
                    .into()
            )
        );
        assert_eq!(
            a.delete_everywhere("neuf"),
            Err(AppError::NotPublished("neuf".to_owned()))
        );
        a.delete_everywhere("perso").unwrap();
        assert!(a.config().unwrap().vault("perso").is_err());
        b.delete_everywhere("perso").unwrap();
        assert!(b.config().unwrap().vault("brouillon").is_ok());
        assert!(b.config().unwrap().vault("perso").is_err());
        let names: Vec<String> = a
            .server_vaults()
            .unwrap()
            .into_iter()
            .map(|v| v.name)
            .collect();
        assert_eq!(names, ["brouillon", "famille"]);
    }

    #[test]
    fn accounts_devices_and_keys_are_administered() {
        let dir = TempDir::new().unwrap();
        let (a, b) = (device(&dir, "a"), device(&dir, "b"));
        let server = connect_admin(&a);
        connect(&b, &server, &a.add_member("Camille").unwrap());
        let camille = account_id(&a, "Camille");

        a.set_admin(camille, true).unwrap();
        assert!(b.server_info().unwrap().is_admin);
        a.set_admin(camille, false).unwrap();
        a.add_member("Dominique").unwrap();
        let keys = a.pending_keys().unwrap();
        assert_eq!(keys[0].name.as_deref(), Some("Dominique"));
        a.cancel_key(keys[0].id).unwrap();
        assert_eq!(a.pending_keys().unwrap(), []);

        let device = a.admin_accounts().unwrap()[1].devices[0].id;
        a.revoke_device(device).unwrap();
        assert!(b.server_info().is_err());
        a.delete_account(camille).unwrap();
        assert_eq!(a.admin_accounts().unwrap().len(), 1);
    }

    #[test]
    fn vaults_are_administered() {
        let dir = TempDir::new().unwrap();
        let (a, b) = (device(&dir, "a"), device(&dir, "b"));
        create(&b, "famille", true);
        let server = connect_admin(&a);
        connect(&b, &server, &a.add_member("Camille").unwrap());
        b.publish_vaults().unwrap();
        let famille = a.server_vaults().unwrap()[0].id;
        let camille = account_id(&a, "Camille");
        let arthur = account_id(&a, "Arthur");

        assert_eq!(a.versions(famille).unwrap()[0].version, 1);
        let restored = a.restore_version(famille, 1).unwrap();
        assert_eq!(restored.name, "famille-v1");
        b.download_vaults().unwrap();
        assert!(b.config().unwrap().vault("famille-v1").is_ok());

        a.remove_member(famille, camille).unwrap();
        a.attach(famille, arthur).unwrap();
        b.download_vaults().unwrap();
        assert_eq!(
            b.vault_states().unwrap()[0],
            ("famille".to_owned(), VaultState::Removed)
        );
        a.delete_server_vault(famille).unwrap();
        assert_eq!(a.server_vaults().unwrap().len(), 1);
    }

    #[test]
    fn the_server_is_backed_up() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        connect_admin(&a);
        let backup = a.backup_server().unwrap();
        let stamp = crate::utc::server_backup_file_name(now());
        assert_eq!(backup.file_name.len(), stamp.len());
        assert!(backup.file_name.starts_with("pswd-server-sauvegarde_"));
        assert!(backup.content.starts_with(b"SQLite format 3\0"));
    }

    #[test]
    fn a_disconnected_device_has_no_panel() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        let id = Uuid::new_v4();
        assert_eq!(a.server_info().err(), Some(AppError::NotConnected));
        assert_eq!(a.backup_server().err(), Some(AppError::NotConnected));
        assert_eq!(a.delete_everywhere("perso"), Err(AppError::NotConnected));
        assert_eq!(a.set_public_address(""), Err(AppError::NotConnected));
        assert_eq!(a.add_member("Camille").err(), Some(AppError::NotConnected));
        for result in [
            a.admin_accounts().map(drop),
            a.delete_account(id),
            a.set_admin(id, true),
            a.revoke_device(id),
            a.pending_keys().map(drop),
            a.cancel_key(id),
            a.server_vaults().map(drop),
            a.delete_server_vault(id),
            a.attach(id, id),
            a.remove_member(id, id),
            a.versions(id).map(drop),
            a.restore_version(id, 1).map(drop),
        ] {
            assert_eq!(result, Err(AppError::NotConnected));
        }
    }
}
