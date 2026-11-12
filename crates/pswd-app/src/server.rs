//! Connexion de l'appareil à un serveur et synchro de ses coffres (SPEC §11).

use std::fmt::Write;

use pswd_core::ports::{LocalStore, PortError, PushError, RemoteStore, RemoteVersion};
use pswd_core::{ConnectionKey, FileError, MAX_VAULT_NAME_LEN, Secret, SyncStatus, VaultFile};
use pswd_infra::{
    Config, FileStore, RemoteLink, RemoteVault, RemoteVaultInfo, Removal, ServerClient,
    ServerConfig, ServerFile, SyncedState, VaultConfig,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::vaults::read_vault_file;
use crate::{AppError, VaultSession, Vaults};

/// Port pris quand l'adresse n'en donne pas.
pub const DEFAULT_PORT: u16 = 8443;

/// État d'un coffre vis-à-vis du serveur de l'appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultState {
    /// Pas sur ce serveur (jamais publié, ou lié à un autre serveur) : publié au prochain
    /// échange.
    Local,
    /// Même contenu que la copie du serveur.
    Synced,
    /// Un seul des deux côtés a changé : le prochain échange le met à jour, sans mot de passe.
    Outdated,
    /// Modifié des deux côtés, ou dernier état commun inconnu : la fusion exige de le
    /// déverrouiller.
    NeedsUnlock,
    /// Absent du compte sur le serveur (supprimé, ou jamais accessible à ce compte) : gardé
    /// ici, republié seulement à la demande.
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    NotConnected,
    /// Le coffre a été supprimé du serveur : il n'est plus synchronisé, sauf s'il est
    /// republié.
    Removed,
    /// Le compte n'est plus membre de ce coffre commun : sa copie locale a été supprimée.
    Left,
    /// Le coffre a été supprimé partout depuis un autre appareil, et cette copie n'avait rien de
    /// plus : elle a été supprimée.
    Deleted,
    /// Un coffre du serveur porte déjà ce nom : ce coffre n'est pas publié. Le prochain échange
    /// le renomme puis le publie (il n'est pas renommé tant qu'il est ouvert).
    NameTaken,
    /// Jamais [`SyncStatus::Removed`] : ce cas devient [`SyncOutcome::Removed`],
    /// [`SyncOutcome::Left`] ou [`SyncOutcome::Deleted`].
    Done(SyncStatus),
}

/// Ce qu'un échange a fait d'un coffre déjà sur le serveur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Update {
    /// La copie locale, plus récente, a été envoyée.
    Sent,
    /// La copie du serveur, plus récente, a été téléchargée.
    Received,
    /// Modifié des deux côtés : il faut le déverrouiller pour fusionner.
    NeedsUnlock,
}

/// Résultat d'un échange avec le serveur, coffre par coffre.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Exchange {
    /// Coffres de l'appareil renommés pour laisser leur nom à un coffre du serveur : ancien et
    /// nouveau nom. Ils sont publiés sous le nouveau.
    pub renamed: Vec<(String, String)>,
    /// Pour chaque coffre du serveur que l'appareil n'avait pas : son nom ici.
    pub downloaded: VaultResults<String>,
    /// Coffres locaux publiés sur le serveur.
    pub published: VaultResults<()>,
    /// Coffres déjà sur le serveur, mis à jour ou à déverrouiller ; ceux qui étaient à jour
    /// n'y figurent pas.
    pub updated: VaultResults<Update>,
    /// Coffres absents du compte sur le serveur depuis le dernier échange : gardés ici (ceux
    /// supprimés partout mais modifiés ici compris).
    pub removed: Vec<String>,
    /// Coffres communs dont le compte n'est plus membre : supprimés de cet appareil.
    pub left: VaultResults<()>,
    /// Coffres supprimés partout depuis un autre appareil, sans modification ici : supprimés de
    /// cet appareil.
    pub deleted: VaultResults<()>,
}

/// Pourquoi un coffre n'a pas été retiré de l'appareil à la déconnexion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeptReason {
    /// La copie locale a des modifications que le serveur n'a pas, ou le coffre n'y est pas.
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

/// Ce que l'échange a fait d'un coffre lié au serveur.
enum Step {
    Unchanged,
    Updated(Update),
    Gone(Removal),
}

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

    /// Échange avec le serveur, sans mot de passe : télécharge les coffres du compte que
    /// l'appareil n'a pas (sauf ceux retirés de l'appareil), publie les coffres locaux, met à
    /// jour ceux dont un seul côté a changé, garde ceux qui sont absents du compte et supprime
    /// ceux dont le compte n'est plus membre, ou supprimés partout et sans modification ici.
    /// Un coffre absent du compte n'est jamais republié ici. L'adresse publique du serveur est
    /// reprise au passage.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté, si le serveur est injoignable ou si la configuration
    /// est illisible : ce qui a déjà été fait est gardé. Les échecs propres à un coffre sont
    /// rangés dans l'[`Exchange`].
    pub fn exchange(&self) -> Result<Exchange, AppError> {
        let server = self.connected()?;
        let client = server.client()?;
        let listed = client.list_vaults()?;
        self.remember_public_address(&server, client.server_info()?.public_address.as_deref())?;
        let server_id = server.server_id();
        let mut exchange = Exchange::default();
        self.forget_ignored_gone(&listed)?;
        self.give_way(&server_id, &listed, &mut exchange)?;
        self.download(&client, &server_id, &listed, &mut exchange)?;
        for (name, vault) in self.config()?.vaults {
            let link = vault.remote.clone().filter(|link| link.server == server_id);
            let Some(link) = link else {
                let published = self.publish(&client, &server, &name, &vault);
                exchange.published.push((name, published));
                continue;
            };
            if link.removed {
                continue;
            }
            let listed = listed.iter().find(|remote| remote.id == link.id);
            match self.update(&client, &name, &vault, &link, listed) {
                Ok(Step::Unchanged) => {}
                Ok(Step::Updated(update)) => exchange.updated.push((name, Ok(update))),
                Ok(Step::Gone(Removal::Left)) => {
                    let deleted = self.delete(&name);
                    exchange.left.push((name, deleted));
                }
                Ok(Step::Gone(Removal::DeletedEverywhere)) if nothing_new_here(&vault, &link) => {
                    let deleted = self.delete(&name);
                    exchange.deleted.push((name, deleted));
                }
                Ok(Step::Gone(_)) => {
                    self.set_removed(&name, true)?;
                    exchange.removed.push(name);
                }
                Err(error) => exchange.updated.push((name, Err(error))),
            }
        }
        Ok(exchange)
    }

    /// Télécharge les coffres du serveur que l'appareil n'a pas encore, sauf ceux retirés de
    /// l'appareil. Un coffre noté absent que le serveur donne de nouveau est de nouveau
    /// synchronisé.
    fn download(
        &self,
        client: &ServerClient,
        server_id: &str,
        listed: &[RemoteVaultInfo],
        exchange: &mut Exchange,
    ) -> Result<(), AppError> {
        for remote in listed {
            if let Some(name) = self.config()?.linked_to(remote.id) {
                let name = name.to_owned();
                if self.set_removed(&name, false)? {
                    exchange.downloaded.push((remote.name.clone(), Ok(name)));
                }
                continue;
            }
            if self.config()?.ignored.contains(&remote.id) {
                continue;
            }
            let mut vault = client.vault(remote.id);
            let Some(file) = vault.fetch()? else {
                continue;
            };
            let link = RemoteLink {
                server: server_id.to_owned(),
                id: remote.id,
                removed: false,
                synced: synced_state(vault.version(), &file.file),
            };
            let name = local_name(&self.config()?, &remote.name, listed);
            let added = self
                .add_vault_file(&name, &file.file, remote.shared, Some(link))
                .map(|()| name);
            exchange.downloaded.push((remote.name.clone(), added));
        }
        Ok(())
    }

    /// Un coffre de l'appareil qui n'est pas sur ce serveur cède son nom à un coffre du serveur
    /// (SPEC §11) : renommé `<nom>-2` (ou le premier numéro libre), il est ensuite publié sous
    /// ce nom. Un nom désigne ainsi le même coffre sur tous les appareils.
    fn give_way(
        &self,
        server_id: &str,
        listed: &[RemoteVaultInfo],
        exchange: &mut Exchange,
    ) -> Result<(), AppError> {
        for (name, vault) in self.config()?.vaults {
            let here = vault.remote.is_some_and(|link| link.server == server_id);
            if here || !listed.iter().any(|remote| remote.name == name) {
                continue;
            }
            let new_name = free_name(&name, &self.config()?, listed);
            self.rename_here(&name, &new_name)?;
            exchange.renamed.push((name, new_name));
        }
        Ok(())
    }

    /// Renomme le coffre `name` sur cet appareil seulement.
    fn rename_here(&self, name: &str, new_name: &str) -> Result<(), AppError> {
        let mut config_file = self.config_file();
        let mut config = config_file.load()?;
        config.rename_vault(name, new_name)?;
        config_file.save(&config)?;
        Ok(())
    }

    /// Oublie les coffres retirés de l'appareil que le serveur ne donne plus.
    fn forget_ignored_gone(&self, listed: &[RemoteVaultInfo]) -> Result<(), AppError> {
        let listed_id = |id: &Uuid| listed.iter().any(|remote| remote.id == *id);
        if self.config()?.ignored.iter().all(listed_id) {
            return Ok(());
        }
        self.update_config(|config| config.ignored.retain(listed_id))?;
        Ok(())
    }

    /// Publie un coffre local comme nouveau coffre du serveur, tel quel : son fichier est déjà
    /// chiffré. Un lien avec un autre serveur est remplacé.
    fn publish(
        &self,
        client: &ServerClient,
        server: &ServerConfig,
        name: &str,
        vault: &VaultConfig,
    ) -> Result<(), AppError> {
        let file = read_vault_file(name, vault)?;
        let id = self.reserve(client, server, name, vault.shared)?;
        let mut remote = client.vault(id);
        remote.push(&file, None).map_err(push_error)?;
        self.remember(name, remote.version(), &file)
    }

    /// Met à jour, sans mot de passe, un coffre lié au serveur dont un seul côté a changé depuis
    /// le dernier état commun. `listed` : sa description par le serveur, `None` s'il ne le
    /// donne plus.
    fn update(
        &self,
        client: &ServerClient,
        name: &str,
        vault: &VaultConfig,
        link: &RemoteLink,
        listed: Option<&RemoteVaultInfo>,
    ) -> Result<Step, AppError> {
        let mut remote = client.vault(link.id);
        let step = self.try_update(&mut remote, name, vault, link, listed);
        // Seul un refus 410 donne une raison : l'erreur est alors ce refus.
        match (step, remote.removal()) {
            (Err(_), Some(removal)) => Ok(Step::Gone(removal)),
            (step, _) => step,
        }
    }

    fn try_update(
        &self,
        remote: &mut RemoteVault,
        name: &str,
        vault: &VaultConfig,
        link: &RemoteLink,
        listed: Option<&RemoteVaultInfo>,
    ) -> Result<Step, AppError> {
        let Some(listed) = listed else {
            // Le serveur ne le donne plus : sa réponse (410) dit pourquoi.
            return remote
                .fetch()
                .map(|_| Step::Unchanged)
                .map_err(AppError::from);
        };
        let local = read_vault_file(name, vault)?;
        let Some(version) = &listed.version else {
            // Réservé sans avoir été publié.
            return self.send(remote, name, &local, None);
        };
        let Some(synced) = &link.synced else {
            // Dernier état commun inconnu : il est retenu si les deux copies sont identiques.
            let same = remote.fetch()?.is_some_and(|file| file.file == local);
            if !same {
                return Ok(Step::Updated(Update::NeedsUnlock));
            }
            self.remember(name, remote.version(), &local)?;
            return Ok(Step::Unchanged);
        };
        match changes(synced, version, Some(&local)) {
            (false, false) => Ok(Step::Unchanged),
            (true, false) => self.send(remote, name, &local, Some(version)),
            (false, true) => self.receive(remote, name, vault),
            (true, true) => Ok(Step::Updated(Update::NeedsUnlock)),
        }
    }

    /// Envoie la copie locale, si le serveur n'a pas bougé depuis `expected`.
    fn send(
        &self,
        remote: &mut RemoteVault,
        name: &str,
        local: &[u8],
        expected: Option<&RemoteVersion>,
    ) -> Result<Step, AppError> {
        remote.push(local, expected).map_err(push_error)?;
        self.remember(name, remote.version(), local)?;
        Ok(Step::Updated(Update::Sent))
    }

    /// Remplace la copie locale, inchangée depuis le dernier état commun, par celle du serveur :
    /// elle contient déjà tout ce qu'avait la copie locale.
    fn receive(
        &self,
        remote: &mut RemoteVault,
        name: &str,
        vault: &VaultConfig,
    ) -> Result<Step, AppError> {
        let file = remote.fetch()?.map(|file| file.file).unwrap_or_default();
        VaultFile::check_format(&file).map_err(rejected)?;
        FileStore::new(vault.vault_file()).save_atomic(&file)?;
        self.remember(name, remote.version(), &file)?;
        Ok(Step::Updated(Update::Received))
    }

    /// Retient le dernier état commun du coffre `name` : la copie locale `file` a le même
    /// contenu que la `version` du serveur (inconnue : l'état commun l'est aussi).
    fn remember(
        &self,
        name: &str,
        version: Option<&RemoteVersion>,
        file: &[u8],
    ) -> Result<(), AppError> {
        let synced = synced_state(version, file);
        let note = |config: &mut Config| {
            let link = config
                .vaults
                .get_mut(name)
                .and_then(|vault| vault.remote.as_mut());
            if let Some(link) = link {
                link.synced = synced;
            }
        };
        self.update_config(note)?;
        Ok(())
    }

    /// Note le coffre `name` comme supprimé du serveur, ou comme rejoint ; renvoie `true` si
    /// cela change son état.
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
            synced: None,
        };
        config.link_vault(name, link)?;
        config_file.save(&config)?;
        Ok(id)
    }

    /// Comme [`Vaults::reserve`], sauf si un coffre du serveur porte déjà ce nom : `None`.
    fn reserve_unless_taken(
        &self,
        client: &ServerClient,
        server: &ServerConfig,
        name: &str,
        shared: bool,
    ) -> Result<Option<Uuid>, AppError> {
        if client
            .list_vaults()?
            .iter()
            .any(|remote| remote.name == name)
        {
            return Ok(None);
        }
        self.reserve(client, server, name, shared).map(Some)
    }

    /// Synchronise le coffre ouvert `name` avec le serveur, après l'y avoir publié s'il n'y est
    /// pas encore. Un coffre supprimé du serveur n'est pas republié : voir
    /// [`Vaults::republish`].
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
        let server_id = server.server_id();
        let id = match &vault.remote {
            Some(link) if link.server == server_id && link.removed => {
                return Ok(SyncOutcome::Removed);
            }
            Some(link) if link.server == server_id => link.id,
            _ => match self.reserve_unless_taken(&client, &server, name, vault.shared) {
                Ok(Some(id)) => id,
                Ok(None) => return Ok(SyncOutcome::NameTaken),
                Err(AppError::Port(error)) => {
                    return Ok(SyncOutcome::Done(SyncStatus::Offline(error)));
                }
                Err(error) => return Err(error),
            },
        };
        let mut remote = client.vault(id);
        match session.sync(&mut remote)? {
            SyncStatus::Removed if remote.removal() == Some(Removal::Left) => {
                self.delete(name)?;
                Ok(SyncOutcome::Left)
            }
            SyncStatus::Removed
                if remote.removal() == Some(Removal::DeletedEverywhere)
                    && vault
                        .remote
                        .as_ref()
                        .is_some_and(|link| nothing_new_here(&vault, link)) =>
            {
                self.delete(name)?;
                Ok(SyncOutcome::Deleted)
            }
            SyncStatus::Removed => {
                self.set_removed(name, true)?;
                Ok(SyncOutcome::Removed)
            }
            SyncStatus::Synced => {
                self.remember(name, remote.version(), session.saved_file())?;
                Ok(SyncOutcome::Done(SyncStatus::Synced))
            }
            status => Ok(SyncOutcome::Done(status)),
        }
    }

    /// Synchronise le coffre ouvert `name` à la demande de l'utilisateur : supprimé du serveur,
    /// il y est republié comme nouveau coffre.
    ///
    /// # Errors
    ///
    /// Comme [`Vaults::sync`].
    pub fn republish(
        &self,
        name: &str,
        session: &mut VaultSession,
    ) -> Result<SyncOutcome, AppError> {
        self.update_config(|config| {
            if let Some(vault) = config.vaults.get_mut(name) {
                vault.remote.take_if(|link| link.removed);
            }
        })?;
        self.sync(name, session)
    }

    /// État de chaque coffre, par nom, d'après la liste des coffres du serveur.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté, si le serveur est injoignable, ou si la configuration
    /// est illisible.
    pub fn vault_states(&self) -> Result<Vec<(String, VaultState)>, AppError> {
        let server = self.connected()?;
        let listed = server.client()?.list_vaults()?;
        let server_id = server.server_id();
        Ok(self
            .config()?
            .vaults
            .iter()
            .map(|(name, vault)| (name.clone(), state(vault, &server_id, &listed)))
            .collect())
    }

    /// `true` si la copie du serveur du coffre `name` a changé depuis leur dernier état commun,
    /// ou si celui-ci est inconnu : une synchro avec fusion est à faire. `false` pour un coffre
    /// inconnu, ou absent de ce serveur (jamais publié, lié à un autre serveur, supprimé).
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté, si le serveur est injoignable, ou si la configuration
    /// est illisible.
    pub fn server_moved(&self, name: &str) -> Result<bool, AppError> {
        let server = self.connected()?;
        let listed = server.client()?.list_vaults()?;
        let server_id = server.server_id();
        Ok(self
            .config()?
            .vaults
            .get(name)
            .and_then(|vault| vault.remote.as_ref())
            .is_some_and(|link| link.server == server_id && moved(link, &listed)))
    }

    /// Retire de cet appareil le coffre publié `name`, qui reste sur le serveur et sur les
    /// autres appareils : il n'est plus téléchargé ici (voir [`Vaults::download_again`]).
    ///
    /// # Errors
    ///
    /// Si le coffre n'est pas publié sur le serveur de l'appareil, si sa copie locale a des
    /// modifications que le serveur n'a pas, ou si le serveur est injoignable : rien n'est
    /// alors retiré.
    pub fn forget(&self, name: &str) -> Result<(), AppError> {
        let server = self.connected()?;
        let vault = self.config()?.vault(name)?.clone();
        let Some(link) = vault
            .remote
            .as_ref()
            .filter(|link| link.server == server.server_id() && !link.removed)
        else {
            return Err(AppError::NotPublished(name.to_owned()));
        };
        if !nothing_only_here(&vault, link, &server.client()?.list_vaults()?)? {
            return Err(AppError::Unpublished(name.to_owned()));
        }
        self.update_config(|config| config.ignored.push(link.id))?;
        self.delete(name)
    }

    /// Coffres du serveur retirés de cet appareil.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté, si le serveur est injoignable, ou si la configuration
    /// est illisible.
    pub fn ignored_vaults(&self) -> Result<Vec<RemoteVaultInfo>, AppError> {
        let listed = self.connected()?.client()?.list_vaults()?;
        let ignored = self.config()?.ignored;
        Ok(listed
            .into_iter()
            .filter(|remote| ignored.contains(&remote.id))
            .collect())
    }

    /// Télécharge de nouveau le coffre `id`, retiré de cet appareil, par un échange.
    ///
    /// # Errors
    ///
    /// Comme [`Vaults::exchange`].
    pub fn download_again(&self, id: Uuid) -> Result<Exchange, AppError> {
        self.connected()?;
        self.update_config(|config| config.ignored.retain(|ignored| *ignored != id))?;
        self.exchange()
    }

    /// Clé de connexion d'un appareil de plus pour le même compte, à montrer une seule fois.
    ///
    /// # Errors
    ///
    /// Si l'appareil n'est pas connecté ou si le serveur est injoignable.
    pub fn new_device_key(&self) -> Result<Secret, AppError> {
        Ok(self.connected()?.client()?.new_device_key()?)
    }

    /// Déconnecte l'appareil : son jeton est révoqué et le serveur oublié, avec les coffres
    /// retirés de l'appareil. Sans `keep_local`, les coffres dont l'appareil n'a rien que le
    /// serveur n'ait déjà sont retirés de l'appareil ; les autres sont gardés. Un serveur
    /// injoignable n'empêche pas la déconnexion.
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
            let listed = client.list_vaults().map_err(AppError::from);
            for (name, vault) in &self.config()?.vaults {
                let Some(link) = &vault.remote else { continue };
                if link.server != server.server_id() || link.removed {
                    continue;
                }
                let known = listed
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|listed| nothing_only_here(vault, link, listed));
                match known {
                    Ok(true) => to_remove.push(name.clone()),
                    Ok(false) => report.kept.push((name.clone(), KeptReason::Unpublished)),
                    Err(error) => report.kept.push((name.clone(), KeptReason::Error(error))),
                }
            }
        }
        report.revoke_error = client.disconnect().err();
        self.forget_server()?;
        for name in to_remove {
            match self.delete(&name) {
                Ok(()) => report.removed.push(name),
                Err(error) => report.kept.push((name, KeptReason::Error(error))),
            }
        }
        Ok(report)
    }

    /// L'appareil oublie le serveur : `server.toml` et la liste des coffres retirés. Les coffres
    /// restent sur l'appareil.
    pub(crate) fn forget_server(&self) -> Result<(), AppError> {
        self.update_config(|config| config.ignored.clear())?;
        self.server_file().remove()?;
        Ok(())
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

fn state(vault: &VaultConfig, server_id: &str, listed: &[RemoteVaultInfo]) -> VaultState {
    let Some(link) = vault
        .remote
        .as_ref()
        .filter(|link| link.server == server_id)
    else {
        return VaultState::Local;
    };
    let listed = listed.iter().find(|remote| remote.id == link.id);
    let Some(listed) = listed.filter(|_| !link.removed) else {
        return VaultState::Removed;
    };
    let Some(version) = &listed.version else {
        return VaultState::Local;
    };
    let Some(synced) = &link.synced else {
        return VaultState::NeedsUnlock;
    };
    let local = FileStore::new(vault.vault_file()).load().ok().flatten();
    match changes(synced, version, local.as_deref()) {
        (false, false) => VaultState::Synced,
        (true, true) => VaultState::NeedsUnlock,
        _ => VaultState::Outdated,
    }
}

/// La copie du serveur a changé depuis le dernier état commun, ou celui-ci est inconnu.
fn moved(link: &RemoteLink, listed: &[RemoteVaultInfo]) -> bool {
    listed
        .iter()
        .find(|remote| remote.id == link.id)
        .and_then(|remote| remote.version.as_ref())
        .is_some_and(|version| {
            link.synced
                .as_ref()
                .is_none_or(|synced| synced.version != version.0)
        })
}

/// Ce qui a changé depuis le dernier état commun : la copie locale (absente, elle compte comme
/// changée), puis celle du serveur, à la `version` donnée.
fn changes(synced: &SyncedState, version: &RemoteVersion, local: Option<&[u8]>) -> (bool, bool) {
    (local_changed(synced, local), synced.version != version.0)
}

fn local_changed(synced: &SyncedState, local: Option<&[u8]>) -> bool {
    local.is_none_or(|file| file_hash(file) != synced.file_hash)
}

fn synced_state(version: Option<&RemoteVersion>, file: &[u8]) -> Option<SyncedState> {
    version.map(|version| SyncedState {
        version: version.0.clone(),
        file_hash: file_hash(file),
    })
}

/// Empreinte SHA-256 d'un fichier, en hexadécimal.
fn file_hash(file: &[u8]) -> String {
    Sha256::digest(file)
        .iter()
        .fold(String::with_capacity(64), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// Nom d'un coffre téléchargé : celui du serveur, ou, s'il est déjà pris ici par un autre coffre
/// du serveur, le premier `<nom>-<numéro>` libre.
fn local_name(config: &Config, name: &str, listed: &[RemoteVaultInfo]) -> String {
    if config.vaults.contains_key(name) {
        free_name(name, config, listed)
    } else {
        name.to_owned()
    }
}

/// Premier nom `<nom>-2`, `<nom>-3`… qui n'est pris ni sur cet appareil, ni sur le serveur.
fn free_name(name: &str, config: &Config, listed: &[RemoteVaultInfo]) -> String {
    let taken = |candidate: &str| {
        config.vaults.contains_key(candidate)
            || listed.iter().any(|remote| remote.name == candidate)
    };
    let mut number = 2;
    loop {
        let candidate = numbered(name, number);
        if !taken(&candidate) {
            return candidate;
        }
        number += 1;
    }
}

/// `<nom>-<numéro>`, le nom raccourci pour ne pas dépasser `MAX_VAULT_NAME_LEN` caractères. Il
/// reste valide : le nom ne commence pas par une espace.
fn numbered(name: &str, number: u32) -> String {
    let suffix = format!("-{number}");
    let prefix: String = name
        .chars()
        .take(MAX_VAULT_NAME_LEN - suffix.len())
        .collect();
    format!("{}{suffix}", prefix.trim_end())
}

/// L'appareil n'a rien que le serveur n'ait déjà : le coffre y est publié, et la copie locale
/// n'a pas changé depuis le dernier état commun. Le serveur a pu avancer depuis : il ne fait
/// qu'ajouter à cet état.
/// La copie locale n'a pas changé depuis le dernier état commun : le serveur avait tout.
fn nothing_new_here(vault: &VaultConfig, link: &RemoteLink) -> bool {
    let local = FileStore::new(vault.vault_file()).load().ok().flatten();
    link.synced
        .as_ref()
        .is_some_and(|synced| !local_changed(synced, local.as_deref()))
}

fn nothing_only_here(
    vault: &VaultConfig,
    link: &RemoteLink,
    listed: &[RemoteVaultInfo],
) -> Result<bool, AppError> {
    let published = listed
        .iter()
        .any(|remote| remote.id == link.id && remote.version.is_some());
    let Some(synced) = link.synced.as_ref().filter(|_| published) else {
        return Ok(false);
    };
    let local = FileStore::new(vault.vault_file()).load()?;
    Ok(!local_changed(synced, local.as_deref()))
}

/// Un envoi sans mot de passe est refusé si le serveur a bougé depuis sa liste : il faudra
/// recommencer, et peut-être fusionner.
fn push_error(error: PushError) -> AppError {
    match error {
        PushError::Conflict => PortError::new(
            "le coffre a changé sur le serveur pendant l'envoi : synchroniser de nouveau",
        )
        .into(),
        PushError::Failed(error) => error.into(),
    }
}

/// Copie du serveur qui n'est pas un `vault.enc` valide : elle ne remplace pas la copie locale.
fn rejected(error: FileError) -> AppError {
    PortError::new(format!("copie du serveur refusée : {error}")).into()
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

    /// Ajoute une note enregistrée localement, sans synchro.
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
        let mut titles: Vec<String> = session
            .vault()
            .entries()
            .map(|entry| entry.content().title.clone())
            .collect();
        titles.sort();
        titles
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

    /// Deux appareils du même compte qui ont chacun le coffre `perso`, à jour.
    fn two_devices(dir: &TempDir) -> (Vaults, Vaults, TestServer) {
        let a = device(dir, "a");
        create(&a, "perso");
        let server = connect_admin(&a);
        a.exchange().unwrap();
        let b = device(dir, "b");
        connect_same_account(&a, &b);
        b.exchange().unwrap();
        (a, b, server)
    }

    fn states(vaults: &Vaults) -> Vec<(String, VaultState)> {
        vaults.vault_states().unwrap()
    }

    fn state_of(name: &str, state: VaultState) -> (String, VaultState) {
        (name.to_owned(), state)
    }

    fn names(vaults: &Vaults) -> Vec<String> {
        vaults.config().unwrap().vaults.into_keys().collect()
    }

    /// Suppression du serveur par un administrateur : les copies locales sont gardées.
    fn delete_on_server(admin: &Vaults, name: &str) {
        admin
            .delete_server_vault(link(admin, name).unwrap().id)
            .unwrap();
    }

    fn ignored(vaults: &Vaults) -> Vec<Uuid> {
        vaults.config().unwrap().ignored
    }

    fn link(vaults: &Vaults, name: &str) -> Option<RemoteLink> {
        vaults.config().unwrap().vault(name).unwrap().remote.clone()
    }

    fn vault_file(vaults: &Vaults, name: &str) -> PathBuf {
        vaults.config().unwrap().vault(name).unwrap().vault_file()
    }

    fn done(name: &str) -> (String, Result<(), AppError>) {
        (name.to_owned(), Ok(()))
    }

    fn updated(name: &str, update: Update) -> (String, Result<Update, AppError>) {
        (name.to_owned(), Ok(update))
    }

    /// Oublie le dernier état commun du coffre, comme une configuration d'avant sa création.
    fn forget_synced(vaults: &Vaults, name: &str) {
        vaults
            .update_config(|config| {
                let vault = config.vaults.get_mut(name).unwrap();
                vault.remote.as_mut().unwrap().synced = None;
            })
            .unwrap();
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
        PortError::unreachable("serveur injoignable").into()
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
        assert_eq!(states(&a), [state_of("perso", VaultState::Local)]);
        let exchange = a.exchange().unwrap();
        assert_eq!(exchange.published, [done("perso")]);
        assert_eq!(states(&a), [state_of("perso", VaultState::Synced)]);
        assert_eq!(a.exchange().unwrap(), Exchange::default());

        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        assert_eq!(
            b.exchange().unwrap().downloaded,
            [("perso".to_owned(), Ok("perso".to_owned()))]
        );
        assert_eq!(states(&b), [state_of("perso", VaultState::Synced)]);
        assert_eq!(titles(&open(&b, "perso")), ["Wi-Fi"]);
        assert_eq!(b.exchange().unwrap(), Exchange::default());
    }

    #[test]
    fn a_local_vault_gives_its_name_to_the_vault_of_the_server() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        add_note(&mut create(&a, "banque"), "depuis a");
        connect_admin(&a);
        a.exchange().unwrap();

        let b = device(&dir, "b");
        add_note(&mut create(&b, "banque"), "depuis b");
        connect_same_account(&a, &b);
        let exchange = b.exchange().unwrap();
        assert_eq!(
            exchange.renamed,
            [("banque".to_owned(), "banque-2".to_owned())]
        );
        assert_eq!(
            exchange.downloaded,
            [("banque".to_owned(), Ok("banque".to_owned()))]
        );
        assert_eq!(exchange.published, [done("banque-2")]);
        assert_eq!(titles(&open(&b, "banque")), ["depuis a"]);
        assert_eq!(titles(&open(&b, "banque-2")), ["depuis b"]);

        let exchange = a.exchange().unwrap();
        assert_eq!(
            exchange.downloaded,
            [("banque-2".to_owned(), Ok("banque-2".to_owned()))]
        );
        assert_eq!(titles(&open(&a, "banque-2")), ["depuis b"]);
        for device in [&a, &b] {
            assert_eq!(
                states(device),
                [
                    state_of("banque", VaultState::Synced),
                    state_of("banque-2", VaultState::Synced)
                ]
            );
        }
    }

    #[test]
    fn the_new_name_is_free_here_and_on_the_server() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        create(&a, "perso-2");
        connect_admin(&a);
        a.exchange().unwrap();

        let b = device(&dir, "b");
        create(&b, "perso");
        create(&b, "perso-3");
        connect_same_account(&a, &b);
        let exchange = b.exchange().unwrap();
        assert_eq!(
            exchange.renamed,
            [("perso".to_owned(), "perso-4".to_owned())]
        );
        assert_eq!(names(&b), ["perso", "perso-2", "perso-3", "perso-4"]);
    }

    #[test]
    fn a_second_vault_of_the_server_with_the_same_name_is_numbered() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.exchange().unwrap();
        let client = a.server().unwrap().unwrap().client().unwrap();
        let file = fs::read(a.config().unwrap().vault("perso").unwrap().vault_file()).unwrap();
        let twin = client.create_vault("perso", false).unwrap();
        client.vault(twin).push(&file, None).unwrap();

        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        let exchange = b.exchange().unwrap();
        assert_eq!(exchange.renamed, []);
        assert_eq!(exchange.downloaded.len(), 2);
        assert_eq!(names(&b), ["perso", "perso-2"]);
        assert_eq!(b.exchange().unwrap(), Exchange::default());
    }

    #[test]
    fn an_open_vault_is_not_published_under_a_name_of_the_server() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.exchange().unwrap();

        let b = device(&dir, "b");
        let mut session = create(&b, "perso");
        connect_same_account(&a, &b);
        assert_eq!(b.sync("perso", &mut session), Ok(SyncOutcome::NameTaken));
        assert_eq!(link(&b, "perso"), None);
        assert_eq!(a.server_vaults().unwrap().len(), 1);
        assert_eq!(
            b.exchange().unwrap().renamed,
            [("perso".to_owned(), "perso-2".to_owned())]
        );
    }

    #[test]
    fn numbered_names_stay_valid() {
        assert_eq!(numbered("perso", 2), "perso-2");
        let kept = MAX_VAULT_NAME_LEN - 3;
        let long = "a".repeat(MAX_VAULT_NAME_LEN);
        assert_eq!(numbered(&long, 10), format!("{}-10", "a".repeat(kept)));
        let space_cut = format!("{} b", "a".repeat(kept));
        assert_eq!(numbered(&space_cut, 2), format!("{}-2", "a".repeat(kept)));
        let heavy = "🔒".repeat(MAX_VAULT_NAME_LEN);
        for number in [2, 10, 1000] {
            assert!(pswd_core::is_valid_vault_name(&numbered(&heavy, number)));
        }
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
        assert_eq!(b.exchange().unwrap(), Exchange::default());
        assert_eq!(states(&b), []);
    }

    #[test]
    fn a_reserved_vault_is_published_by_the_exchange() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        let id = a
            .server()
            .unwrap()
            .unwrap()
            .client()
            .unwrap()
            .create_vault("perso", false)
            .unwrap();
        let mut config = a.config().unwrap();
        let reserved = RemoteLink {
            server: a.server().unwrap().unwrap().server_id(),
            id,
            removed: false,
            synced: None,
        };
        config.link_vault("perso", reserved).unwrap();
        a.config_file().save(&config).unwrap();
        assert_eq!(states(&a), [state_of("perso", VaultState::Local)]);
        assert_eq!(
            a.exchange().unwrap().updated,
            [updated("perso", Update::Sent)]
        );
        assert_eq!(states(&a), [state_of("perso", VaultState::Synced)]);
    }

    #[test]
    fn a_failed_download_is_reported_per_vault() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.exchange().unwrap();
        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        let data = dir.path().join("b").join("data");
        fs::write(data.join("vaults"), b"").unwrap();
        let results = b.exchange().unwrap().downloaded;
        assert_eq!(results.len(), 1);
        assert!(
            matches!(results[0].1, Err(AppError::Port(_))),
            "{results:?}"
        );
        assert_eq!(names(&b), Vec::<String>::new());
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
        let results = a.exchange().unwrap().published;
        assert_eq!(
            results[0],
            (
                "abime".to_owned(),
                Err(PortError::new("réponse inattendue du serveur (400)").into())
            )
        );
        assert_eq!(results[1].0, "absent");
        assert!(matches!(results[1].1, Err(AppError::File(_))));
        assert_eq!(results[2], done("perso"));
    }

    #[test]
    fn an_offline_device_keeps_its_vaults_local() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        go_offline(&a);
        assert_eq!(a.exchange(), Err(offline()));
        assert_eq!(a.vault_states(), Err(offline()));
        assert_eq!(link(&a, "perso"), None);
    }

    #[test]
    fn a_disconnected_device_has_nothing_to_exchange() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        assert_eq!(a.exchange(), Err(AppError::NotConnected));
        assert_eq!(a.vault_states(), Err(AppError::NotConnected));
        assert_eq!(a.server_moved("perso"), Err(AppError::NotConnected));
        assert_eq!(a.new_device_key(), Err(AppError::NotConnected));
        assert_eq!(a.disconnect(true), Err(AppError::NotConnected));
        let mut session = open(&a, "perso");
        assert_eq!(a.sync("perso", &mut session), Ok(SyncOutcome::NotConnected));
        assert_eq!(link(&a, "perso"), None);
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
        assert_eq!(states(&a), [state_of("perso", VaultState::Synced)]);

        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        b.exchange().unwrap();
        let mut on_b = open(&b, "perso");
        add_note(&mut on_b, "Banque");
        assert_eq!(
            b.sync("perso", &mut on_b),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
        assert_eq!(states(&b), [state_of("perso", VaultState::Synced)]);
        assert_eq!(
            a.sync("perso", &mut on_a),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
        assert_eq!(titles(&on_a), ["Banque", "Wi-Fi"]);
        assert_eq!(states(&a), [state_of("perso", VaultState::Synced)]);
    }

    #[test]
    fn a_local_change_is_sent_without_password() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        add_note(&mut open(&b, "perso"), "Banque");
        assert_eq!(states(&b), [state_of("perso", VaultState::Outdated)]);
        assert_eq!(
            b.exchange().unwrap().updated,
            [updated("perso", Update::Sent)]
        );
        assert_eq!(states(&b), [state_of("perso", VaultState::Synced)]);

        assert_eq!(states(&a), [state_of("perso", VaultState::Outdated)]);
        assert_eq!(
            a.exchange().unwrap().updated,
            [updated("perso", Update::Received)]
        );
        assert_eq!(states(&a), [state_of("perso", VaultState::Synced)]);
        assert_eq!(titles(&open(&a, "perso")), ["Banque"]);
    }

    #[test]
    fn changes_on_both_sides_need_the_password() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        add_note(&mut open(&a, "perso"), "Wi-Fi");
        a.exchange().unwrap();
        let mut on_b = open(&b, "perso");
        add_note(&mut on_b, "Banque");
        assert_eq!(states(&b), [state_of("perso", VaultState::NeedsUnlock)]);
        assert_eq!(
            b.exchange().unwrap().updated,
            [updated("perso", Update::NeedsUnlock)]
        );
        assert_eq!(
            b.sync("perso", &mut on_b),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
        assert_eq!(titles(&on_b), ["Banque", "Wi-Fi"]);
        assert_eq!(states(&b), [state_of("perso", VaultState::Synced)]);
    }

    #[test]
    fn an_unknown_common_state_is_learned_from_identical_copies() {
        let dir = TempDir::new().unwrap();
        let (_a, b, _server) = two_devices(&dir);
        forget_synced(&b, "perso");
        assert_eq!(states(&b), [state_of("perso", VaultState::NeedsUnlock)]);
        assert_eq!(b.exchange().unwrap(), Exchange::default());
        assert_eq!(states(&b), [state_of("perso", VaultState::Synced)]);

        add_note(&mut open(&b, "perso"), "Banque");
        forget_synced(&b, "perso");
        assert_eq!(
            b.exchange().unwrap().updated,
            [updated("perso", Update::NeedsUnlock)]
        );
        assert_eq!(states(&b), [state_of("perso", VaultState::NeedsUnlock)]);
    }

    #[test]
    fn only_a_change_on_the_server_calls_for_a_sync() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        create(&b, "neuf");
        assert_eq!(b.server_moved("perso"), Ok(false));
        assert_eq!(b.server_moved("neuf"), Ok(false));
        assert_eq!(b.server_moved("inconnu"), Ok(false));

        add_note(&mut open(&b, "perso"), "Banque");
        assert_eq!(b.server_moved("perso"), Ok(false));
        b.exchange().unwrap();
        assert_eq!(a.server_moved("perso"), Ok(true));
        a.sync("perso", &mut open(&a, "perso")).unwrap();
        assert_eq!(a.server_moved("perso"), Ok(false));

        forget_synced(&a, "perso");
        assert_eq!(a.server_moved("perso"), Ok(true));
        b.delete_everywhere("perso").unwrap();
        assert_eq!(a.server_moved("perso"), Ok(false));
    }

    #[test]
    fn a_vault_of_another_server_has_not_moved_on_this_one() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.exchange().unwrap();
        a.disconnect(true).unwrap();
        connect_admin(&a);
        assert_eq!(a.server_moved("perso"), Ok(false));
        go_offline(&a);
        assert_eq!(a.server_moved("perso"), Err(offline()));
    }

    #[test]
    fn a_missing_local_copy_is_reported() {
        let dir = TempDir::new().unwrap();
        let (_a, b, _server) = two_devices(&dir);
        fs::remove_file(vault_file(&b, "perso")).unwrap();
        assert_eq!(states(&b), [state_of("perso", VaultState::Outdated)]);
        let results = b.exchange().unwrap().updated;
        assert_eq!(results[0].0, "perso");
        assert!(matches!(results[0].1, Err(AppError::File(_))));
    }

    #[test]
    fn a_vault_deleted_from_the_server_is_kept_until_republished() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        delete_on_server(&a, "perso");
        let exchange = b.exchange().unwrap();
        assert_eq!(exchange.removed, ["perso"]);
        assert_eq!(states(&b), [state_of("perso", VaultState::Removed)]);
        assert_eq!(b.exchange().unwrap(), Exchange::default());

        let mut session = open(&b, "perso");
        assert_eq!(b.sync("perso", &mut session), Ok(SyncOutcome::Removed));
        assert_eq!(
            b.republish("perso", &mut session),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
        assert_eq!(states(&b), [state_of("perso", VaultState::Synced)]);
        assert_eq!(
            b.republish("autre", &mut session),
            Err(ConfigError::UnknownVault("autre".to_owned()).into())
        );
    }

    #[test]
    fn a_vault_deleted_from_the_server_is_neither_shared_nor_removed_at_disconnection() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        delete_on_server(&a, "perso");
        b.exchange().unwrap();
        assert_eq!(
            b.members("perso").err(),
            Some(AppError::NotPublished("perso".to_owned()))
        );
        let report = b.disconnect(false).unwrap();
        assert_eq!((report.removed, report.kept), (vec![], vec![]));
        assert_eq!(names(&b), ["perso"]);
    }

    #[test]
    fn an_offline_sync_of_a_published_vault_is_reported() {
        let dir = TempDir::new().unwrap();
        let (a, _b, _server) = two_devices(&dir);
        let mut session = open(&a, "perso");
        go_offline(&a);
        assert_eq!(
            a.sync("perso", &mut session),
            Ok(SyncOutcome::Done(SyncStatus::Offline(
                PortError::unreachable("serveur injoignable")
            )))
        );
    }

    #[test]
    fn an_open_vault_deleted_from_the_server_is_kept() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        let mut session = open(&b, "perso");
        delete_on_server(&a, "perso");
        assert_eq!(b.sync("perso", &mut session), Ok(SyncOutcome::Removed));
        assert_eq!(states(&b), [state_of("perso", VaultState::Removed)]);
        assert_eq!(names(&b), ["perso"]);
    }

    #[test]
    fn a_vault_deleted_everywhere_leaves_the_other_devices() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        a.delete_everywhere("perso").unwrap();
        let exchange = b.exchange().unwrap();
        assert_eq!(exchange.deleted, [done("perso")]);
        assert_eq!(exchange.removed, Vec::<String>::new());
        assert_eq!(names(&b), Vec::<String>::new());
    }

    #[test]
    fn a_vault_deleted_everywhere_but_changed_here_is_kept() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        add_note(&mut open(&b, "perso"), "pas encore publiée");
        a.delete_everywhere("perso").unwrap();
        let exchange = b.exchange().unwrap();
        assert_eq!(exchange.removed, ["perso"]);
        assert_eq!(exchange.deleted, []);
        assert_eq!(states(&b), [state_of("perso", VaultState::Removed)]);
        assert_eq!(b.exchange().unwrap(), Exchange::default());
    }

    #[test]
    fn a_vault_deleted_everywhere_with_an_unknown_common_state_is_kept() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        forget_synced(&b, "perso");
        a.delete_everywhere("perso").unwrap();
        assert_eq!(b.exchange().unwrap().removed, ["perso"]);
        assert_eq!(names(&b), ["perso"]);
    }

    #[test]
    fn an_open_vault_deleted_everywhere_is_deleted_unless_changed() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        create(&a, "travail");
        a.exchange().unwrap();
        b.exchange().unwrap();
        let mut perso = open(&b, "perso");
        let mut travail = open(&b, "travail");
        add_note(&mut travail, "pas encore publiée");
        a.delete_everywhere("perso").unwrap();
        a.delete_everywhere("travail").unwrap();
        assert_eq!(b.sync("perso", &mut perso), Ok(SyncOutcome::Deleted));
        assert_eq!(b.sync("travail", &mut travail), Ok(SyncOutcome::Removed));
        assert_eq!(names(&b), ["travail"]);
        assert!(link(&b, "travail").unwrap().removed);
    }

    #[test]
    fn a_vault_removed_from_this_device_stays_on_the_server() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        let id = link(&b, "perso").unwrap().id;
        b.forget("perso").unwrap();
        assert_eq!(names(&b), Vec::<String>::new());
        assert_eq!(ignored(&b), [id]);
        assert_eq!(b.exchange().unwrap(), Exchange::default());
        assert_eq!(names(&b), Vec::<String>::new());
        assert_eq!(states(&a), [state_of("perso", VaultState::Synced)]);
        let listed: Vec<String> = b
            .ignored_vaults()
            .unwrap()
            .into_iter()
            .map(|remote| remote.name)
            .collect();
        assert_eq!(listed, ["perso"]);

        let exchange = b.download_again(id).unwrap();
        assert_eq!(
            exchange.downloaded,
            [("perso".to_owned(), Ok("perso".to_owned()))]
        );
        assert_eq!(ignored(&b), Vec::<Uuid>::new());
        assert_eq!(b.ignored_vaults().unwrap(), []);
    }

    #[test]
    fn only_a_published_copy_without_changes_is_removed_from_the_device() {
        let dir = TempDir::new().unwrap();
        let (_a, b, _server) = two_devices(&dir);
        create(&b, "local");
        assert_eq!(
            b.forget("local"),
            Err(AppError::NotPublished("local".to_owned()))
        );
        add_note(&mut open(&b, "perso"), "pas encore publiée");
        assert_eq!(
            b.forget("perso"),
            Err(AppError::Unpublished("perso".to_owned()))
        );
        assert_eq!(names(&b), ["local", "perso"]);
        assert_eq!(ignored(&b), Vec::<Uuid>::new());
        assert_eq!(
            b.forget("inconnu"),
            Err(ConfigError::UnknownVault("inconnu".to_owned()).into())
        );
        b.update_config(|config| {
            let elsewhere = config.vaults["perso"].remote.clone().map(|mut remote| {
                remote.server = "autre serveur".to_owned();
                remote
            });
            config.vaults.get_mut("local").unwrap().remote = elsewhere;
        })
        .unwrap();
        assert_eq!(
            b.forget("local"),
            Err(AppError::NotPublished("local".to_owned()))
        );
        go_offline(&b);
        assert_eq!(b.forget("perso"), Err(offline()));
    }

    #[test]
    fn vaults_removed_from_the_device_are_forgotten_once_gone() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        create(&a, "travail");
        a.exchange().unwrap();
        b.exchange().unwrap();
        let travail = link(&b, "travail").unwrap().id;
        b.forget("perso").unwrap();
        b.forget("travail").unwrap();
        a.delete_everywhere("perso").unwrap();
        b.exchange().unwrap();
        assert_eq!(ignored(&b), [travail]);
        b.disconnect(true).unwrap();
        assert_eq!(ignored(&b), Vec::<Uuid>::new());
    }

    #[test]
    fn a_disconnected_device_has_no_vault_removed_from_it() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        assert_eq!(a.ignored_vaults(), Err(AppError::NotConnected));
        assert_eq!(
            a.download_again(Uuid::new_v4()),
            Err(AppError::NotConnected)
        );
        assert_eq!(a.forget("perso"), Err(AppError::NotConnected));
    }

    #[test]
    fn a_vault_given_again_is_synced_again() {
        let dir = TempDir::new().unwrap();
        let (_a, b, _server) = two_devices(&dir);
        b.set_removed("perso", true).unwrap();
        assert_eq!(states(&b), [state_of("perso", VaultState::Removed)]);
        assert_eq!(
            b.exchange().unwrap().downloaded,
            [("perso".to_owned(), Ok("perso".to_owned()))]
        );
        assert_eq!(states(&b), [state_of("perso", VaultState::Synced)]);
        assert!(!b.set_removed("inconnu", true).unwrap());
    }

    #[test]
    fn a_vault_out_of_the_account_is_reported_once_and_kept() {
        let dir = TempDir::new().unwrap();
        let (_a, b, _server) = two_devices(&dir);
        let out_of_the_account = |vaults: &Vaults, name: &str| {
            vaults
                .update_config(|config| {
                    let vault = config.vaults.get_mut(name).unwrap();
                    vault.remote.as_mut().unwrap().id = Uuid::new_v4();
                })
                .unwrap();
        };
        out_of_the_account(&b, "perso");
        assert_eq!(states(&b), [state_of("perso", VaultState::Removed)]);
        let exchange = b.exchange().unwrap();
        assert_eq!(exchange.removed, ["perso"]);
        assert_eq!(exchange.updated, []);
        // Le vrai coffre du compte, que le lien ne désigne plus, est téléchargé à côté.
        assert_eq!(exchange.downloaded.len(), 1);
        assert_eq!(b.exchange().unwrap(), Exchange::default());
        assert!(names(&b).contains(&"perso".to_owned()));

        let mut session = open(&b, "perso");
        assert_eq!(b.sync("perso", &mut session), Ok(SyncOutcome::Removed));
        // Le vrai « perso » du serveur garde son nom : celui-ci le lui laisse au prochain échange.
        assert_eq!(
            b.republish("perso", &mut session),
            Ok(SyncOutcome::NameTaken)
        );
        assert_eq!(link(&b, "perso"), None);
        assert_eq!(names(&b), ["perso", "perso-2"]);
        let exchange = b.exchange().unwrap();
        assert_eq!(
            exchange.renamed,
            [("perso".to_owned(), "perso-3".to_owned())]
        );
        assert_eq!(exchange.published, [done("perso-3")]);

        out_of_the_account(&b, "perso-3");
        assert_eq!(b.sync("perso-3", &mut session), Ok(SyncOutcome::Removed));
        assert!(link(&b, "perso-3").unwrap().removed);
    }

    #[test]
    fn the_common_state_of_an_unknown_vault_is_not_noted() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        let before = a.config().unwrap();
        a.remember("inconnu", Some(&RemoteVersion("\"1\"".to_owned())), b"")
            .unwrap();
        assert_eq!(a.config().unwrap(), before);
    }

    #[test]
    fn offline_sync_keeps_the_vault_local() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        let mut session = create(&a, "perso");
        connect_admin(&a);
        go_offline(&a);
        assert_eq!(
            a.sync("perso", &mut session),
            Ok(SyncOutcome::Done(SyncStatus::Offline(
                PortError::unreachable("serveur injoignable")
            )))
        );
        assert_eq!(link(&a, "perso"), None);
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
    fn vaults_of_another_server_are_published_on_this_one() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        let mut session = create(&a, "perso");
        create(&a, "travail");
        connect_admin(&a);
        a.exchange().unwrap();
        a.disconnect(true).unwrap();
        let old = link(&a, "perso").unwrap();

        connect_admin(&a);
        assert_eq!(
            states(&a),
            [
                state_of("perso", VaultState::Local),
                state_of("travail", VaultState::Local)
            ]
        );
        assert_eq!(
            a.sync("perso", &mut session),
            Ok(SyncOutcome::Done(SyncStatus::Synced))
        );
        assert_ne!(link(&a, "perso").unwrap().server, old.server);
        assert_eq!(a.exchange().unwrap().published, [done("travail")]);
        assert_eq!(
            states(&a),
            [
                state_of("perso", VaultState::Synced),
                state_of("travail", VaultState::Synced)
            ]
        );
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
        assert!(matches!(
            a.server_moved("perso"),
            Err(AppError::Config(ConfigError::Invalid(_)))
        ));
    }

    #[test]
    fn disconnection_keeps_local_copies_by_default() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.exchange().unwrap();
        let client = a.server().unwrap().unwrap().client().unwrap();
        assert_eq!(a.disconnect(true), Ok(Disconnected::default()));
        assert!(a.server().unwrap().is_none());
        assert_eq!(names(&a), ["perso"]);
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
        a.exchange().unwrap();
        let address = a.server().unwrap().unwrap().local_address;
        let device_key = key(&a.new_device_key().unwrap());
        a.disconnect(true).unwrap();
        a.connect(&address, &device_key, "PC", None).unwrap();
        assert_eq!(states(&a), [state_of("perso", VaultState::Synced)]);
        assert_eq!(a.exchange().unwrap(), Exchange::default());
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
        a.exchange().unwrap();
        create(&a, "nouveau");
        add_note(&mut open(&a, "modifie"), "pas encore publiée");
        let removed_dir = vault_file(&a, "perso").parent().unwrap().to_owned();
        let mut other = a.server().unwrap().unwrap();
        other.fingerprint[0] ^= 1;
        let mut config = a.config().unwrap();
        let mut moved = config.vault("local").unwrap().remote.clone().unwrap();
        moved.server = other.server_id();
        config.link_vault("local", moved).unwrap();
        a.config_file().save(&config).unwrap();

        let report = a.disconnect(false).unwrap();
        assert_eq!(report.revoke_error, None);
        assert_eq!(report.removed, ["perso"]);
        assert_eq!(
            report.kept,
            [("modifie".to_owned(), KeptReason::Unpublished)]
        );
        assert!(!removed_dir.exists());
        assert_eq!(names(&a), ["local", "modifie", "nouveau"]);
    }

    #[test]
    fn disconnection_removes_a_copy_that_is_only_behind_the_server() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        add_note(&mut open(&b, "perso"), "depuis b");
        b.exchange().unwrap();
        let report = a.disconnect(false).unwrap();
        assert_eq!(report.removed, ["perso"]);
        assert_eq!(report.kept, []);
        assert_eq!(names(&a), Vec::<String>::new());
    }

    #[test]
    fn disconnection_keeps_a_vault_the_server_no_longer_lists() {
        let dir = TempDir::new().unwrap();
        let (a, b, _server) = two_devices(&dir);
        b.delete_everywhere("perso").unwrap();
        let report = a.disconnect(false).unwrap();
        assert_eq!(report.removed, Vec::<String>::new());
        assert_eq!(report.kept, [("perso".to_owned(), KeptReason::Unpublished)]);
        assert_eq!(names(&a), ["perso"]);
    }

    #[test]
    fn offline_disconnection_keeps_every_vault() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        create(&a, "perso");
        connect_admin(&a);
        a.exchange().unwrap();
        go_offline(&a);
        let report = a.disconnect(false).unwrap();
        assert_eq!(
            report.revoke_error,
            Some(PortError::unreachable("serveur injoignable"))
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
        a.exchange().unwrap();
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
    fn devices_of_an_account_share_their_vaults() {
        let dir = TempDir::new().unwrap();
        let a = device(&dir, "a");
        connect_admin(&a);
        let b = device(&dir, "b");
        connect_same_account(&a, &b);
        create(&b, "perso");
        b.exchange().unwrap();
        assert_eq!(
            a.exchange().unwrap().downloaded,
            [("perso".to_owned(), Ok("perso".to_owned()))]
        );
    }

    #[test]
    fn push_errors_are_explained() {
        assert_eq!(
            push_error(PushError::Conflict),
            PortError::new(
                "le coffre a changé sur le serveur pendant l'envoi : synchroniser de nouveau"
            )
            .into()
        );
        assert_eq!(
            push_error(PushError::Failed(PortError::unreachable(
                "serveur injoignable"
            ))),
            offline()
        );
    }

    #[test]
    fn an_invalid_server_copy_is_refused() {
        assert_eq!(
            rejected(FileError::Corrupted),
            PortError::new("copie du serveur refusée : coffre altéré ou corrompu").into()
        );
    }

    #[test]
    fn file_hashes_are_sha256_in_hexadecimal() {
        assert_eq!(
            file_hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
