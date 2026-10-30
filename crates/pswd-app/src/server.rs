//! Connexion de l'appareil à un serveur et synchro de ses coffres (SPEC §11).

use pswd_core::ports::{LocalStore, PortError, PushError, RemoteStore};
use pswd_core::{ConnectionKey, MAX_VAULT_NAME_LEN, Secret, SyncStatus};
use pswd_infra::{
    Config, FileStore, RemoteLink, ServerClient, ServerConfig, ServerFile, VaultConfig,
};
use uuid::Uuid;

use crate::vaults::read_vault_file;
use crate::{AppError, VaultSession, Vaults};

/// Port pris quand l'adresse n'en donne pas.
pub const DEFAULT_PORT: u16 = 8443;

/// Nombre de caractères de l'identifiant ajoutés au nom d'un coffre téléchargé déjà pris.
const SHORT_ID_LEN: usize = 8;

/// État d'un coffre sur cet appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultState {
    /// Inconnu du serveur : publié à la prochaine occasion.
    ToPublish,
    /// Lié au serveur de l'appareil.
    Published,
    /// Lié à un autre serveur, ou appareil déconnecté : utilisé en local.
    Detached,
    /// Le serveur l'a retiré, ou en a retiré le compte : gardé en local, jamais republié.
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    NotConnected,
    Detached,
    /// Le coffre est retiré (§11) : il n'est plus synchronisé.
    Removed,
    /// Jamais [`SyncStatus::Removed`] : ce cas devient [`SyncOutcome::Removed`].
    Done(SyncStatus),
}

/// Résultat d'un téléchargement des coffres du compte.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Downloads {
    /// Pour chaque coffre du serveur : son nom sur cet appareil.
    pub downloaded: VaultResults<String>,
    /// Coffres que le serveur a retirés depuis le dernier échange ; gardés en local.
    pub removed: Vec<String>,
}

/// Pourquoi un coffre n'a pas été retiré de l'appareil à la déconnexion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeptReason {
    /// La copie locale diffère de celle du serveur.
    Unpublished,
    Error(AppError),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Disconnected {
    /// Le jeton n'a pas pu être révoqué : à révoquer depuis le panneau d'administration.
    pub revoke_error: Option<PortError>,
    pub removed: Vec<String>,
    pub kept: Vec<(String, KeptReason)>,
}

/// Résultat pour chaque coffre, désigné par son nom.
pub type VaultResults<T> = Vec<(String, Result<T, AppError>)>;

impl Vaults {
    pub(crate) fn server_file(&self) -> ServerFile {
        ServerFile::new(self.locations.server_file())
    }

    /// Serveur auquel l'appareil est connecté.
    ///
    /// # Errors
    ///
    /// Si `server.toml` existe mais ne peut pas être lu.
    pub fn server(&self) -> Result<Option<ServerConfig>, AppError> {
        Ok(self.server_file().load()?)
    }

    pub(crate) fn connected(&self) -> Result<ServerConfig, AppError> {
        self.server()?.ok_or(AppError::NotConnected)
    }

    /// Connecte l'appareil au serveur `address` (`hôte` ou `hôte:port`) avec une clé de
    /// connexion, et retient ce serveur. `account_name` nomme le compte créé par une clé
    /// administrateur.
    ///
    /// # Errors
    ///
    /// Si l'appareil est déjà connecté, si le serveur est injoignable, n'est pas celui de la clé
    /// ou la refuse, ou si `server.toml` ne peut pas être écrit.
    pub fn connect(
        &self,
        address: &str,
        key: &ConnectionKey,
        device_name: &str,
        account_name: Option<&str>,
    ) -> Result<(), AppError> {
        let mut server_file = self.server_file();
        if server_file.load()?.is_some() {
            return Err(AppError::AlreadyConnected);
        }
        let address = with_default_port(address);
        let connection =
            ServerClient::connect(vec![address.clone()], key, device_name, account_name)?;
        let server = ServerConfig {
            local_address: address,
            public_address: None,
            fingerprint: connection.fingerprint,
            token: connection.token,
        };
        server_file.save(&server)?;
        Ok(())
    }

    /// Télécharge les coffres publiés du compte que l'appareil n'a pas encore, et note comme
    /// retirés ceux que le serveur ne lui donne plus. L'adresse publique du serveur est reprise
    /// au passage. Un coffre retiré que le compte a rejoint
    /// est de nouveau synchronisé.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté ou si le serveur est injoignable : les coffres déjà
    /// téléchargés sont gardés.
    pub fn download_vaults(&self) -> Result<Downloads, AppError> {
        let server = self.connected()?;
        let client = server.client()?;
        let listed = client.list_vaults()?;
        self.remember_public_address(&server, client.server_info()?.public_address.as_deref())?;
        let server_id = server.server_id();
        let mut downloads = Downloads::default();
        let mark_removed = |config: &mut Config| {
            for (name, vault) in &mut config.vaults {
                let Some(link) = vault.remote.as_mut() else {
                    continue;
                };
                let listed = listed.iter().any(|remote| remote.id == link.id);
                if link.server == server_id && !link.removed && !listed {
                    link.removed = true;
                    downloads.removed.push(name.clone());
                }
            }
        };
        self.update_config(mark_removed)?;
        for remote in listed {
            if let Some(name) = self.config()?.linked_to(remote.id) {
                let name = name.to_owned();
                if self.set_removed(&name, false)? {
                    downloads.downloaded.push((remote.name, Ok(name)));
                }
                continue;
            }
            let Some(file) = client.vault(remote.id).fetch()? else {
                continue;
            };
            let link = RemoteLink {
                server: server_id.clone(),
                id: remote.id,
                removed: false,
            };
            let added = local_name(&self.config()?, &remote.name, remote.id).and_then(|name| {
                self.add_vault_file(&name, &file.file, remote.shared, Some(link))
                    .map(|()| name)
            });
            downloads.downloaded.push((remote.name, added));
        }
        Ok(downloads)
    }

    /// Note le coffre `name` comme retiré, ou comme rejoint ; renvoie `true` si cela change son
    /// état.
    pub(crate) fn set_removed(&self, name: &str, removed: bool) -> Result<bool, AppError> {
        let mut changed = false;
        let mark = |config: &mut Config| {
            let link = config
                .vaults
                .get_mut(name)
                .and_then(|vault| vault.remote.as_mut());
            if let Some(link) = link {
                changed = link.removed != removed;
                link.removed = removed;
            }
        };
        self.update_config(mark)?;
        Ok(changed)
    }

    /// Publie les coffres inconnus du serveur, tels quels : leur fichier est déjà chiffré.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté ou si la configuration est illisible.
    pub fn publish_vaults(&self) -> Result<VaultResults<()>, AppError> {
        let server = self.connected()?;
        let client = server.client()?;
        let config = self.config()?;
        Ok(config
            .vaults
            .iter()
            .filter(|(_, vault)| vault.remote.is_none())
            .map(|(name, vault)| (name.clone(), self.publish(&client, &server, name, vault)))
            .collect())
    }

    fn publish(
        &self,
        client: &ServerClient,
        server: &ServerConfig,
        name: &str,
        vault: &VaultConfig,
    ) -> Result<(), AppError> {
        let file = read_vault_file(name, vault)?;
        let id = self.reserve(client, server, name, vault.shared)?;
        client.vault(id).push(&file, None).map_err(push_error)
    }

    /// Réserve un identifiant sur le serveur et le note aussitôt : une coupure avant la
    /// publication ne crée jamais de doublon.
    fn reserve(
        &self,
        client: &ServerClient,
        server: &ServerConfig,
        name: &str,
        shared: bool,
    ) -> Result<Uuid, AppError> {
        let id = client.create_vault(name, shared)?;
        let mut config_file = self.config_file();
        let mut config = config_file.load()?;
        let link = RemoteLink {
            server: server.server_id(),
            id,
            removed: false,
        };
        config.link_vault(name, link)?;
        config_file.save(&config)?;
        Ok(id)
    }

    /// Synchronise le coffre ouvert `name` avec le serveur, après l'y avoir publié s'il ne le
    /// connaît pas encore.
    ///
    /// # Errors
    ///
    /// Si la configuration est illisible, ou si l'écriture locale échoue ; un serveur injoignable
    /// est signalé par le [`SyncStatus`] renvoyé.
    pub fn sync(&self, name: &str, session: &mut VaultSession) -> Result<SyncOutcome, AppError> {
        let Some(server) = self.server()? else {
            return Ok(SyncOutcome::NotConnected);
        };
        let vault = self.config()?.vault(name)?.clone();
        let client = server.client()?;
        let id = match vault.remote {
            Some(link) if link.removed => return Ok(SyncOutcome::Removed),
            Some(link) if link.server == server.server_id() => link.id,
            Some(_) => return Ok(SyncOutcome::Detached),
            None => match self.reserve(&client, &server, name, vault.shared) {
                Ok(id) => id,
                Err(AppError::Port(error)) => {
                    return Ok(SyncOutcome::Done(SyncStatus::Offline(error)));
                }
                Err(error) => return Err(error),
            },
        };
        match session.sync(&mut client.vault(id))? {
            SyncStatus::Removed => {
                self.set_removed(name, true)?;
                Ok(SyncOutcome::Removed)
            }
            status => Ok(SyncOutcome::Done(status)),
        }
    }

    /// État de chaque coffre, par nom.
    ///
    /// # Errors
    ///
    /// Si la configuration ou `server.toml` est illisible.
    pub fn vault_states(&self) -> Result<Vec<(String, VaultState)>, AppError> {
        let server_id = self.server()?.map(|server| server.server_id());
        Ok(self
            .config()?
            .vaults
            .iter()
            .map(|(name, vault)| (name.clone(), state(vault, server_id.as_deref())))
            .collect())
    }

    /// Clé de connexion d'un appareil de plus pour le même compte, à montrer une seule fois.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté ou si le serveur est injoignable.
    pub fn new_device_key(&self) -> Result<Secret, AppError> {
        Ok(self.connected()?.client()?.new_device_key()?)
    }

    /// Déconnecte l'appareil : son jeton est révoqué et le serveur oublié. Sans `keep_local`, les
    /// coffres dont la copie locale est identique à celle du serveur sont retirés de l'appareil ;
    /// les autres sont gardés. Un serveur injoignable n'empêche pas la déconnexion.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté, ou si `server.toml` ne peut pas être supprimé.
    pub fn disconnect(&self, keep_local: bool) -> Result<Disconnected, AppError> {
        let server = self.connected()?;
        let client = server.client()?;
        let mut report = Disconnected::default();
        let mut to_remove = Vec::new();
        if !keep_local {
            for (name, vault) in &self.config()?.vaults {
                let Some(link) = &vault.remote else { continue };
                if link.server != server.server_id() || link.removed {
                    continue;
                }
                match same_as_server(&client, link.id, vault) {
                    Ok(true) => to_remove.push(name.clone()),
                    Ok(false) => report.kept.push((name.clone(), KeptReason::Unpublished)),
                    Err(error) => report.kept.push((name.clone(), KeptReason::Error(error))),
                }
            }
        }
        report.revoke_error = client.disconnect().err();
        self.server_file().remove()?;
        for name in to_remove {
            match self.delete(&name) {
                Ok(()) => report.removed.push(name),
                Err(error) => report.kept.push((name, KeptReason::Error(error))),
            }
        }
        Ok(report)
    }
}

pub(crate) fn with_default_port(address: &str) -> String {
    let address = address.trim();
    if address.contains(':') {
        address.to_owned()
    } else {
        format!("{address}:{DEFAULT_PORT}")
    }
}

fn state(vault: &VaultConfig, server_id: Option<&str>) -> VaultState {
    match (&vault.remote, server_id) {
        (None, _) => VaultState::ToPublish,
        (Some(link), _) if link.removed => VaultState::Removed,
        (Some(link), Some(id)) if link.server == id => VaultState::Published,
        (Some(_), _) => VaultState::Detached,
    }
}

/// Nom d'un coffre téléchargé : celui du serveur, ou, s'il est pris, suivi du début de
/// l'identifiant.
fn local_name(config: &Config, name: &str, id: Uuid) -> Result<String, AppError> {
    if config.ensure_available(name).is_ok() {
        return Ok(name.to_owned());
    }
    let short: String = id.simple().to_string().chars().take(SHORT_ID_LEN).collect();
    let prefix: String = name
        .chars()
        .take(MAX_VAULT_NAME_LEN - SHORT_ID_LEN - 1)
        .collect();
    let alternative = format!("{prefix}-{short}");
    config.ensure_available(&alternative)?;
    Ok(alternative)
}

fn same_as_server(client: &ServerClient, id: Uuid, vault: &VaultConfig) -> Result<bool, AppError> {
    let local = FileStore::new(vault.vault_file()).load()?;
    let remote = client.vault(id).fetch()?;
    Ok(matches!((local, remote), (Some(local), Some(remote)) if local == remote.file))
}

fn push_error(error: PushError) -> AppError {
    match error {
        PushError::Conflict => PortError::new("le coffre a déjà été publié sur le serveur").into(),
        PushError::Failed(error) => error.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::TcpListener;
    use std::path::PathBuf;

    use pswd_core::crypto::KdfParams;
    use pswd_core::{Credentials, EntryContent, EntryKind, Timestamp};
    use pswd_infra::{ConfigError, Locations};
    use pswd_server::testing::TestServer;
    use tempfile::TempDir;

    use super::*;

    const PASSWORD: &str = "Maître-2026!";

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    fn device(dir: &TempDir, device: &str) -> Vaults {
        let root = dir.path().join(device);
        Vaults::new(Locations::new(&root.join("config"), &root.join("data")))
    }

    fn create(vaults: &Vaults, name: &str) -> VaultSession {
        let params = KdfParams::new(8 * 1024, 1, 1).unwrap();
        vaults
            .create(name, &secret(PASSWORD), params, Timestamp::from_millis(0))
            .unwrap()
            .0
    }

    fn open(vaults: &Vaults, name: &str) -> VaultSession {
        vaults
            .open(name, Credentials::Password(&secret(PASSWORD)))
            .unwrap()
    }

    fn add_note(session: &mut VaultSession, title: &str) {
        session
            .edit(|vault| {
                vault.add(
                    EntryContent::new(EntryKind::Note, title.to_owned()),
                    Timestamp::from_millis(1),
                )
            })
            .unwrap();
    }

    fn titles(session: &VaultSession) -> Vec<String> {
        session
            .vault()
            .entries()
            .map(|entry| entry.content().title.clone())
            .collect()
    }

    fn key(text: &Secret) -> ConnectionKey {
        ConnectionKey::parse(text.expose()).unwrap()
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

    /// Connecte `other` au compte de `connected`.
    fn connect_same_account(connected: &Vaults, other: &Vaults) {
        let address = connected.server().unwrap().unwrap().local_address;
        let device_key = key(&connected.new_device_key().unwrap());
        other
            .connect(&address, &device_key, "Portable", None)
            .unwrap();
    }

    fn states(vaults: &Vaults) -> Vec<(String, VaultState)> {
        vaults.vault_states().unwrap()
    }

    fn state_of(name: &str, state: VaultState) -> (String, VaultState) {
        (name.to_owned(), state)
    }

    fn vault_file(vaults: &Vaults, name: &str) -> PathBuf {
        vaults.config().unwrap().vault(name).unwrap().vault_file()
    }

    /// Adresse qui coupe toute connexion. Un port simplement libéré pourrait être repris par
    /// le serveur d'un autre test.
    fn closed_address() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for connection in listener.incoming() {
                drop(connection);
            }
        });
        address
    }

    /// Fait pointer `server.toml` vers une adresse où rien ne répond.
    fn go_offline(vaults: &Vaults) {
        let mut server = vaults.server().unwrap().unwrap();
        server.local_address = closed_address();
        vaults.server_file().save(&server).unwrap();
    }

    fn offline() -> AppError {
        PortError::new("serveur injoignable").into()
    }

    #[test]
    fn connection_remembers_the_server() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        assert!(vaults.server().unwrap().is_none());
        let server = connect_admin(&vaults);
        let saved = vaults.server().unwrap().unwrap();
        assert_eq!(saved.local_address, server.address);
        assert_eq!(saved.fingerprint, server.fingerprint);
        assert_eq!(saved.token.expose().len(), 64);
    }

    #[test]
    fn a_connected_device_cannot_connect_again() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        let server = connect_admin(&vaults);
        assert_eq!(
            vaults.connect(
                &server.address,
                &key(&server.admin_key),
                "PC",
                Some("Arthur")
            ),
            Err(AppError::AlreadyConnected)
        );
    }

    #[test]
    fn refused_connection_leaves_the_device_disconnected() {
        let dir = TempDir::new().unwrap();
        let vaults = device(&dir, "a");
        let server = TestServer::start();
        assert_eq!(
            vaults.connect(&server.address, &key(&server.admin_key), "PC", None),
            Err(PortError::new("nom d'appareil ou de compte invalide").into())
        );
        assert!(vaults.server().unwrap().is_none());
    }

    #[test]
    fn address_gets_the_default_port() {
        assert_eq!(with_default_port(" pswd.local "), "pswd.local:8443");
        assert_eq!(with_default_port("192.168.1.10:9000"), "192.168.1.10:9000");
    }

    #[test]
    fn local_vaults_are_published_then_downloaded_by_another_device() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        let mut session = create(&a, "perso");
        add_note(&mut session, "Wi-Fi");
        connect_admin(&a);
        assert_eq!(states(&a), [state_of("perso", VaultState::ToPublish)]);
        assert_eq!(a.publish_vaults().unwrap(), [("perso".to_owned(), Ok(()))]);
        assert_eq!(states(&a), [state_of("perso", VaultState::Published)]);
        assert_eq!(a.publish_vaults().unwrap(), []);

        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        assert_eq!(
            b.download_vaults().unwrap().downloaded,
            [("perso".to_owned(), Ok("perso".to_owned()))]
        );
        assert_eq!(states(&b), [state_of("perso", VaultState::Published)]);
        assert_eq!(titles(&open(&b, "perso")), ["Wi-Fi"]);
        assert_eq!(b.download_vaults().unwrap(), Downloads::default());
    }

    #[test]
    fn a_taken_name_is_followed_by_the_identifier() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.publish_vaults().unwrap();
        let id = a
            .config()
            .unwrap()
            .vault("perso")
            .unwrap()
            .remote
            .clone()
            .unwrap()
            .id;

        let b = device(&dir, "b");
        create(&b, "perso");
        connect_same_account(&a, &b);
        let expected = format!("perso-{}", &id.simple().to_string()[..8]);
        assert_eq!(
            b.download_vaults().unwrap().downloaded,
            [("perso".to_owned(), Ok(expected.clone()))]
        );
        assert_eq!(
            states(&b),
            [
                state_of("perso", VaultState::ToPublish),
                state_of(&expected, VaultState::Published)
            ]
        );
    }

    #[test]
    fn downloaded_names_stay_valid() {
        let id = Uuid::parse_str("0123456789abcdef0123456789abcdef").unwrap();
        let mut config = Config::default();
        let long = "a".repeat(MAX_VAULT_NAME_LEN);
        for name in ["perso", long.as_str()] {
            config
                .add_vault(
                    name,
                    VaultConfig {
                        path: PathBuf::from(name),
                        shared: false,
                        remote: None,
                    },
                )
                .unwrap();
        }
        assert_eq!(local_name(&config, "travail", id), Ok("travail".to_owned()));
        assert_eq!(
            local_name(&config, "perso", id),
            Ok("perso-01234567".to_owned())
        );
        assert_eq!(
            local_name(&config, &long, id),
            Ok(format!("{}-01234567", "a".repeat(55)))
        );
        config
            .add_vault(
                "perso-01234567",
                VaultConfig {
                    path: PathBuf::from("x"),
                    shared: false,
                    remote: None,
                },
            )
            .unwrap();
        assert_eq!(
            local_name(&config, "perso", id),
            Err(ConfigError::VaultExists("perso-01234567".to_owned()).into())
        );
    }

    #[test]
    fn unpublished_reservations_are_not_downloaded() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        connect_admin(&a);
        a.server()
            .unwrap()
            .unwrap()
            .client()
            .unwrap()
            .create_vault("vide", false)
            .unwrap();
        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        assert_eq!(b.download_vaults().unwrap(), Downloads::default());
        assert_eq!(states(&b), []);
    }

    #[test]
    fn a_failed_download_is_reported_per_vault() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.publish_vaults().unwrap();
        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        let data = dir.path().join("b").join("data");
        fs::write(data.join("vaults"), b"").unwrap();
        let results = b.download_vaults().unwrap().downloaded;
        assert_eq!(results.len(), 1);
        assert!(
            matches!(results[0].1, Err(AppError::Port(_))),
            "{results:?}"
        );
        assert_eq!(states(&b), []);
    }

    #[test]
    fn publication_failures_are_reported_per_vault() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "abime");
        create(&a, "absent");
        create(&a, "perso");
        connect_admin(&a);
        fs::write(vault_file(&a, "abime"), b"pas un coffre").unwrap();
        fs::remove_file(vault_file(&a, "absent")).unwrap();
        let results = a.publish_vaults().unwrap();
        assert_eq!(
            results[0],
            (
                "abime".to_owned(),
                Err(PortError::new("réponse inattendue du serveur (400)").into())
            )
        );
        assert_eq!(results[1].0, "absent");
        assert!(matches!(results[1].1, Err(AppError::File(_))));
        assert_eq!(results[2], ("perso".to_owned(), Ok(())));
    }

    #[test]
    fn offline_device_keeps_its_vaults_to_publish() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        go_offline(&a);
        assert_eq!(
            a.publish_vaults().unwrap(),
            [("perso".to_owned(), Err(offline()))]
        );
        assert_eq!(a.download_vaults(), Err(offline()));
        assert_eq!(states(&a), [state_of("perso", VaultState::ToPublish)]);
    }

    #[test]
    fn a_disconnected_device_has_nothing_to_exchange() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        assert_eq!(a.publish_vaults(), Err(AppError::NotConnected));
        assert_eq!(a.download_vaults(), Err(AppError::NotConnected));
        assert_eq!(a.new_device_key(), Err(AppError::NotConnected));
        assert_eq!(a.disconnect(true), Err(AppError::NotConnected));
        let mut session = open(&a, "perso");
        assert_eq!(a.sync("perso", &mut session), Ok(SyncOutcome::NotConnected));
        assert_eq!(states(&a), [state_of("perso", VaultState::ToPublish)]);
    }

    #[test]
    fn sync_publishes_then_exchanges_changes() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        let mut on_a = create(&a, "perso");
        connect_admin(&a);
        add_note(&mut on_a, "Wi-Fi");
        assert_eq!(
            a.sync("perso", &mut on_a),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
        assert_eq!(states(&a), [state_of("perso", VaultState::Published)]);

        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        b.download_vaults().unwrap();
        let mut on_b = open(&b, "perso");
        add_note(&mut on_b, "Banque");
        assert_eq!(
            b.sync("perso", &mut on_b),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
        assert_eq!(
            a.sync("perso", &mut on_a),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
        let mut titles = titles(&on_a);
        titles.sort();
        assert_eq!(titles, ["Banque", "Wi-Fi"]);
    }

    #[test]
    fn offline_sync_keeps_the_vault_to_publish() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        let mut session = create(&a, "perso");
        connect_admin(&a);
        go_offline(&a);
        assert_eq!(
            a.sync("perso", &mut session),
            Ok(SyncOutcome::Done(SyncStatus::Offline(PortError::new(
                "serveur injoignable"
            ))))
        );
        assert_eq!(states(&a), [state_of("perso", VaultState::ToPublish)]);
    }

    #[test]
    fn sync_reports_configuration_errors() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        let mut session = create(&a, "perso");
        connect_admin(&a);
        assert_eq!(
            a.sync("autre", &mut session),
            Err(ConfigError::UnknownVault("autre".to_owned()).into())
        );
        let config = a.locations.config_file().to_owned();
        let mut blocked = config.into_os_string();
        blocked.push(".tmp");
        fs::create_dir(&blocked).unwrap();
        assert!(matches!(
            a.sync("perso", &mut session),
            Err(AppError::Config(ConfigError::File(_)))
        ));
    }

    #[test]
    fn vaults_of_another_server_are_detached() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        let mut session = create(&a, "perso");
        connect_admin(&a);
        a.publish_vaults().unwrap();
        a.disconnect(true).unwrap();
        connect_admin(&a);
        assert_eq!(states(&a), [state_of("perso", VaultState::Detached)]);
        assert_eq!(a.sync("perso", &mut session), Ok(SyncOutcome::Detached));
        assert_eq!(a.publish_vaults().unwrap(), []);
        assert_eq!(a.download_vaults().unwrap(), Downloads::default());
        assert_eq!(states(&a), [state_of("perso", VaultState::Detached)]);
    }

    #[test]
    fn states_need_a_readable_configuration() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        connect_admin(&a);
        let config = a.locations.config_file().to_owned();
        fs::create_dir_all(config.parent().unwrap()).unwrap();
        fs::write(&config, "couleur = 1").unwrap();
        assert!(matches!(
            a.vault_states(),
            Err(AppError::Config(ConfigError::Invalid(_)))
        ));
    }

    #[test]
    fn disconnection_keeps_local_copies_by_default() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.publish_vaults().unwrap();
        let client = a.server().unwrap().unwrap().client().unwrap();
        assert_eq!(a.disconnect(true), Ok(Disconnected::default()));
        assert!(a.server().unwrap().is_none());
        assert_eq!(states(&a), [state_of("perso", VaultState::Detached)]);
        assert_eq!(
            client.list_vaults(),
            Err(PortError::new(
                "appareil non reconnu par le serveur : déconnecté ou révoqué"
            ))
        );
    }

    #[test]
    fn reconnecting_to_the_same_server_resumes_the_sync() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        let mut session = create(&a, "perso");
        connect_admin(&a);
        a.publish_vaults().unwrap();
        let address = a.server().unwrap().unwrap().local_address;
        let device_key = key(&a.new_device_key().unwrap());
        a.disconnect(true).unwrap();
        a.connect(&address, &device_key, "PC", None).unwrap();
        assert_eq!(states(&a), [state_of("perso", VaultState::Published)]);
        assert_eq!(a.download_vaults().unwrap(), Downloads::default());
        assert_eq!(
            a.sync("perso", &mut session),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
    }

    #[test]
    fn disconnection_without_copies_removes_only_what_the_server_has() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "local");
        create(&a, "modifie");
        create(&a, "perso");
        connect_admin(&a);
        a.publish_vaults().unwrap();
        create(&a, "nouveau");
        add_note(&mut open(&a, "modifie"), "pas encore publiée");
        let removed_dir = vault_file(&a, "perso").parent().unwrap().to_owned();
        let mut other = a.server().unwrap().unwrap();
        other.fingerprint[0] ^= 1;
        let mut config = a.config().unwrap();
        let mut link = config.vault("local").unwrap().remote.clone().unwrap();
        link.server = other.server_id();
        config.link_vault("local", link).unwrap();
        a.config_file().save(&config).unwrap();

        let report = a.disconnect(false).unwrap();
        assert_eq!(report.revoke_error, None);
        assert_eq!(report.removed, ["perso"]);
        assert_eq!(
            report.kept,
            [("modifie".to_owned(), KeptReason::Unpublished)]
        );
        assert!(!removed_dir.exists());
        assert_eq!(
            states(&a),
            [
                state_of("local", VaultState::Detached),
                state_of("modifie", VaultState::Detached),
                state_of("nouveau", VaultState::ToPublish),
            ]
        );
    }

    #[test]
    fn offline_disconnection_keeps_every_vault() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.publish_vaults().unwrap();
        go_offline(&a);
        let report = a.disconnect(false).unwrap();
        assert_eq!(
            report.revoke_error,
            Some(PortError::new("serveur injoignable"))
        );
        assert_eq!(report.removed, Vec::<String>::new());
        assert_eq!(
            report.kept,
            [("perso".to_owned(), KeptReason::Error(offline()))]
        );
        assert!(a.server().unwrap().is_none());
    }

    #[test]
    fn a_vault_that_cannot_be_removed_is_kept() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.publish_vaults().unwrap();
        let file = vault_file(&a, "perso");
        let mut temporary = file.into_os_string();
        temporary.push(".tmp");
        fs::create_dir(&temporary).unwrap();
        let report = a.disconnect(false).unwrap();
        assert_eq!(report.removed, Vec::<String>::new());
        assert_eq!(report.kept.len(), 1);
        assert_eq!(report.kept[0].0, "perso");
        assert!(matches!(
            report.kept[0].1,
            KeptReason::Error(AppError::Port(_))
        ));
    }

    #[test]
    fn server_file_must_be_removable_to_disconnect() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        connect_admin(&a);
        let mut temporary = a.locations.server_file().to_owned().into_os_string();
        temporary.push(".tmp");
        fs::create_dir(&temporary).unwrap();
        assert!(matches!(
            a.disconnect(true),
            Err(AppError::Config(ConfigError::File(_)))
        ));
    }

    #[test]
    fn devices_of_an_account_share_its_vaults() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        connect_admin(&a);
        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        create(&b, "perso");
        b.publish_vaults().unwrap();
        assert_eq!(
            a.download_vaults().unwrap().downloaded,
            [("perso".to_owned(), Ok("perso".to_owned()))]
        );
    }

    #[test]
    fn push_errors_are_explained() {
        assert_eq!(
            push_error(PushError::Conflict),
            PortError::new("le coffre a déjà été publié sur le serveur").into()
        );
        assert_eq!(
            push_error(PushError::Failed(PortError::new("serveur injoignable"))),
            offline()
        );
    }
}
