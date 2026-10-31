//! Coffres communs : inviter, accepter ou refuser, annuler une invitation, quitter (SPEC §11).

use pswd_infra::{Account, ReceivedInvitation, ServerClient, ServerConfig, VaultMembers};
use uuid::Uuid;

use crate::{AppError, Exchange, Vaults};

impl Vaults {
    /// Comptes du serveur, pour choisir qui inviter.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté ou si le serveur est injoignable.
    pub fn accounts(&self) -> Result<Vec<Account>, AppError> {
        Ok(self.client()?.0.accounts()?)
    }

    /// Membres du coffre commun `name` et invitations en attente.
    ///
    /// # Errors
    ///
    /// Si le coffre n'est pas publié sur le serveur de l'appareil, ou si le serveur est
    /// injoignable.
    pub fn members(&self, name: &str) -> Result<VaultMembers, AppError> {
        let (client, server) = self.client()?;
        Ok(client.members(self.published_id(&server, name)?)?)
    }

    /// Invite le compte `account` dans le coffre commun `name`.
    ///
    /// # Errors
    ///
    /// Si le coffre n'est pas publié ou est privé, si le compte est inconnu, déjà membre ou
    /// déjà invité, ou si le serveur est injoignable.
    pub fn invite(&self, name: &str, account: Uuid) -> Result<(), AppError> {
        let (client, server) = self.client()?;
        client.invite(self.published_id(&server, name)?, account)?;
        Ok(())
    }

    /// Invitations reçues par le compte de l'appareil.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté ou si le serveur est injoignable.
    pub fn invitations(&self) -> Result<Vec<ReceivedInvitation>, AppError> {
        Ok(self.client()?.0.invitations()?)
    }

    /// Accepte l'invitation, puis échange avec le serveur pour télécharger le coffre : il
    /// s'ouvre avec le mot de passe que donne celui qui a invité.
    ///
    /// # Errors
    ///
    /// Si l'invitation n'existe plus, ou si le serveur est injoignable.
    pub fn accept_invitation(&self, invitation: Uuid) -> Result<Exchange, AppError> {
        self.client()?.0.accept_invitation(invitation)?;
        self.exchange()
    }

    /// Refuse l'invitation reçue, ou annule celle d'un coffre dont le compte est membre.
    ///
    /// # Errors
    ///
    /// Si l'invitation n'existe plus, ou si le serveur est injoignable.
    pub fn delete_invitation(&self, invitation: Uuid) -> Result<(), AppError> {
        Ok(self.client()?.0.delete_invitation(invitation)?)
    }

    /// Le compte quitte le coffre commun `name`, qui est supprimé de cet appareil ; ses autres
    /// appareils le suppriment au prochain échange. Un coffre déjà quitté depuis un autre
    /// appareil est seulement supprimé ici.
    ///
    /// # Errors
    ///
    /// Si le coffre n'est pas publié ou est privé, ou si le serveur est injoignable : rien n'est
    /// alors supprimé.
    pub fn leave(&self, name: &str) -> Result<(), AppError> {
        let (client, server) = self.client()?;
        match client.leave(self.published_id(&server, name)?) {
            Err(error) if !error.is_removed() => return Err(error.into()),
            _ => {}
        }
        self.delete(name)
    }

    pub(crate) fn client(&self) -> Result<(ServerClient, ServerConfig), AppError> {
        let server = self.connected()?;
        Ok((server.client()?, server))
    }

    /// Identifiant sur le serveur du coffre `name`, publié sur le serveur de l'appareil.
    pub(crate) fn published_id(&self, server: &ServerConfig, name: &str) -> Result<Uuid, AppError> {
        match &self.config()?.vault(name)?.remote {
            Some(link) if link.server == server.server_id() && !link.removed => Ok(link.id),
            _ => Err(AppError::NotPublished(name.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use pswd_core::crypto::KdfParams;
    use pswd_core::ports::PortError;
    use pswd_core::{ConnectionKey, Credentials, Secret, SyncStatus, Timestamp};
    use pswd_infra::{ConfigError, Locations};
    use pswd_server::testing::TestServer;
    use tempfile::TempDir;

    use super::*;
    use crate::{SyncOutcome, VaultSession, VaultState};

    const PASSWORD: &str = "Maître-2026!";

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

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
        let password = secret(PASSWORD);
        if shared {
            vaults.create_shared(name, &password, params, at).unwrap();
        } else {
            vaults.create(name, &password, params, at).unwrap();
        }
    }

    fn open(vaults: &Vaults, name: &str) -> VaultSession {
        vaults
            .open(name, Credentials::Password(&secret(PASSWORD)))
            .unwrap()
    }

    fn state(vaults: &Vaults, name: &str) -> VaultState {
        vaults
            .vault_states()
            .unwrap()
            .into_iter()
            .find(|(vault, _)| vault == name)
            .unwrap()
            .1
    }

    /// Arthur, administrateur, a publié son coffre commun `famille` ; Camille est membre du
    /// même serveur, sans coffre.
    fn family(dir: &TempDir) -> (Vaults, Vaults, TestServer) {
        let server = TestServer::start();
        let arthur = device(dir, "arthur");
        create(&arthur, "famille", true);
        arthur
            .connect(
                &server.address,
                &key(&server.admin_key),
                "PC",
                Some("Arthur"),
            )
            .unwrap();
        arthur.exchange().unwrap();
        let camille = device(dir, "camille");
        let member_key = arthur
            .server()
            .unwrap()
            .unwrap()
            .client()
            .unwrap()
            .add_member("Camille")
            .unwrap();
        camille
            .connect(&server.address, &key(&member_key), "Téléphone", None)
            .unwrap();
        (arthur, camille, server)
    }

    fn account(vaults: &Vaults, name: &str) -> Uuid {
        vaults
            .accounts()
            .unwrap()
            .into_iter()
            .find(|account| account.name == name)
            .unwrap()
            .id
    }

    /// Arthur invite Camille, qui accepte.
    fn join(arthur: &Vaults, camille: &Vaults) -> Exchange {
        arthur
            .invite("famille", account(arthur, "Camille"))
            .unwrap();
        let invitation = camille.invitations().unwrap()[0].id;
        camille.accept_invitation(invitation).unwrap()
    }

    #[test]
    fn an_accepted_invitation_brings_the_shared_vault() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = family(&dir);
        arthur
            .invite("famille", account(&arthur, "Camille"))
            .unwrap();
        let members = arthur.members("famille").unwrap();
        assert_eq!(members.invitations[0].account.name, "Camille");
        let received = camille.invitations().unwrap();
        assert_eq!(received[0].vault_name, "famille");
        assert_eq!(received[0].invited_by, "Arthur");

        let downloads = camille.accept_invitation(received[0].id).unwrap();
        assert_eq!(
            downloads.downloaded,
            [("famille".to_owned(), Ok("famille".to_owned()))]
        );
        assert_eq!(state(&camille, "famille"), VaultState::Synced);
        assert!(camille.config().unwrap().vault("famille").unwrap().shared);
        let mut session = open(&camille, "famille");
        assert_eq!(
            camille.sync("famille", &mut session),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
        assert_eq!(camille.members("famille").unwrap().members.len(), 2);
    }

    #[test]
    fn invitations_are_refused_or_cancelled() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = family(&dir);
        let camille_id = account(&arthur, "Camille");
        arthur.invite("famille", camille_id).unwrap();
        camille
            .delete_invitation(camille.invitations().unwrap()[0].id)
            .unwrap();
        arthur.invite("famille", camille_id).unwrap();
        let sent = arthur.members("famille").unwrap().invitations[0].id;
        arthur.delete_invitation(sent).unwrap();
        assert_eq!(camille.invitations().unwrap(), []);
        assert!(matches!(
            camille.accept_invitation(sent),
            Err(AppError::Port(_))
        ));
    }

    fn names(vaults: &Vaults) -> Vec<String> {
        vaults.config().unwrap().vaults.into_keys().collect()
    }

    /// Un appareil de plus pour le compte de `connected`, qui a téléchargé ses coffres.
    fn other_device(dir: &TempDir, connected: &Vaults, server: &TestServer, name: &str) -> Vaults {
        let other = device(dir, name);
        let device_key = connected.new_device_key().unwrap();
        other
            .connect(&server.address, &key(&device_key), name, None)
            .unwrap();
        other.exchange().unwrap();
        other
    }

    #[test]
    fn a_member_who_leaves_loses_the_local_copy() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = family(&dir);
        join(&arthur, &camille);
        camille.leave("famille").unwrap();
        assert_eq!(names(&camille), Vec::<String>::new());
        assert_eq!(arthur.members("famille").unwrap().members.len(), 1);
        assert_eq!(
            camille.leave("famille"),
            Err(ConfigError::UnknownVault("famille".to_owned()).into())
        );
        assert_eq!(camille.exchange().unwrap(), Exchange::default());
    }

    #[test]
    fn a_vault_of_another_server_is_not_shared() {
        let dir = TempDir::new().unwrap();
        let (arthur, _camille, _server) = family(&dir);
        arthur
            .update_config(|config| {
                let vault = config.vaults.get_mut("famille").unwrap();
                vault.remote.as_mut().unwrap().server = "00".repeat(32);
            })
            .unwrap();
        assert_eq!(
            arthur.members("famille").err(),
            Some(AppError::NotPublished("famille".to_owned()))
        );
    }

    #[test]
    fn other_devices_of_the_account_lose_the_vault() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, server) = family(&dir);
        join(&arthur, &camille);
        let laptop = other_device(&dir, &camille, &server, "portable");
        let tablet = other_device(&dir, &camille, &server, "tablette");
        let mut session = open(&tablet, "famille");
        camille.leave("famille").unwrap();

        assert_eq!(
            laptop.exchange().unwrap().left,
            [("famille".to_owned(), Ok(()))]
        );
        assert_eq!(names(&laptop), Vec::<String>::new());
        assert_eq!(laptop.exchange().unwrap(), Exchange::default());

        assert_eq!(tablet.sync("famille", &mut session), Ok(SyncOutcome::Left));
        assert_eq!(names(&tablet), Vec::<String>::new());
    }

    #[test]
    fn a_vault_left_on_another_device_is_just_deleted() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, server) = family(&dir);
        join(&arthur, &camille);
        let laptop = other_device(&dir, &camille, &server, "portable");
        camille.leave("famille").unwrap();
        assert_eq!(laptop.leave("famille"), Ok(()));
        assert_eq!(names(&laptop), Vec::<String>::new());
    }

    #[test]
    fn a_member_removed_by_an_administrator_loses_the_vault() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = family(&dir);
        join(&arthur, &camille);
        let famille = arthur.server_vaults().unwrap()[0].id;
        arthur
            .remove_member(famille, account(&arthur, "Camille"))
            .unwrap();
        assert_eq!(state(&camille, "famille"), VaultState::Removed);
        assert_eq!(
            camille.exchange().unwrap().left,
            [("famille".to_owned(), Ok(()))]
        );
        assert_eq!(names(&camille), Vec::<String>::new());
    }

    #[test]
    fn a_member_invited_again_gets_the_vault_back() {
        let dir = TempDir::new().unwrap();
        let (arthur, camille, _server) = family(&dir);
        join(&arthur, &camille);
        camille.leave("famille").unwrap();
        let exchange = join(&arthur, &camille);
        assert_eq!(
            exchange.downloaded,
            [("famille".to_owned(), Ok("famille".to_owned()))]
        );
        assert_eq!(state(&camille, "famille"), VaultState::Synced);
    }

    #[test]
    fn only_published_shared_vaults_are_shared() {
        let dir = TempDir::new().unwrap();
        let (arthur, _camille, _server) = family(&dir);
        let camille_id = account(&arthur, "Camille");
        create(&arthur, "brouillon", true);
        let unpublished = Err(AppError::NotPublished("brouillon".to_owned()));
        assert_eq!(arthur.invite("brouillon", camille_id), unpublished);
        assert_eq!(arthur.members("brouillon").err(), unpublished.err());
        create(&arthur, "perso", false);
        arthur.exchange().unwrap();
        assert_eq!(
            arthur.invite("perso", camille_id),
            Err(PortError::new("invitation impossible : coffre privé ou compte inconnu").into())
        );
        assert_eq!(
            arthur.leave("perso"),
            Err(PortError::new("un coffre privé ne se quitte pas").into())
        );
        assert_eq!(state(&arthur, "perso"), VaultState::Synced);
    }

    #[test]
    fn sharing_needs_a_connection() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "seul");
        let id = Uuid::new_v4();
        assert_eq!(vaults.accounts(), Err(AppError::NotConnected));
        assert_eq!(
            vaults.members("famille").err(),
            Some(AppError::NotConnected)
        );
        assert_eq!(vaults.invite("famille", id), Err(AppError::NotConnected));
        assert_eq!(vaults.invitations(), Err(AppError::NotConnected));
        assert_eq!(vaults.accept_invitation(id), Err(AppError::NotConnected));
        assert_eq!(vaults.delete_invitation(id), Err(AppError::NotConnected));
        assert_eq!(vaults.leave("famille"), Err(AppError::NotConnected));
    }
}
