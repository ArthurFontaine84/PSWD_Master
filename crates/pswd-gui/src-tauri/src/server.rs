//! Connexion de l'appareil à un serveur, depuis l'écran « Serveur » (SPEC §11).

use std::collections::BTreeMap;

use pswd_app::{AppError, KeptReason, Update, VaultState};
use pswd_core::{ConnectionKey, Secret, Timestamp};
use serde::Serialize;

use uuid::Uuid;

use crate::error::UiError;
use crate::state::{AppState, confirm};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServerStatus {
    /// `None` si aucun serveur n'est enregistré sur l'appareil.
    address: Option<String>,
    /// Le serveur enregistré ne répond plus ; `None` si le dernier contact a réussi.
    trouble: Option<ServerTrouble>,
    vaults: Vec<VaultStatus>,
    /// Coffres renommés par l'échange automatique qui vient d'avoir lieu ; absent si aucun.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    renamed: Vec<Renamed>,
}

/// Échec du dernier contact avec le serveur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServerTrouble {
    /// Premier échec de la série, en millisecondes depuis 1970.
    pub(crate) since: Timestamp,
    /// Message du dernier échec, en français.
    reason: String,
}

impl ServerTrouble {
    pub(crate) fn new(since: Timestamp, reason: String) -> Self {
        Self { since, reason }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct VaultStatus {
    name: String,
    /// État vis-à-vis du serveur ; `None` si l'appareil n'est pas connecté ou si le serveur est
    /// injoignable.
    state: Option<&'static str>,
    /// Coffre commun, à partager avec d'autres comptes du serveur.
    shared: bool,
}

/// Ce qu'un échange avec le serveur a fait de chaque coffre.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Exchange {
    /// Coffres de cet appareil renommés pour laisser leur nom à un coffre du serveur.
    renamed: Vec<Renamed>,
    downloaded: Vec<Downloaded>,
    not_downloaded: Vec<Failure>,
    published: Vec<String>,
    not_published: Vec<Failure>,
    /// Copie locale, plus récente, envoyée au serveur sans mot de passe.
    sent: Vec<String>,
    /// Copie du serveur, plus récente, téléchargée sans mot de passe.
    received: Vec<String>,
    /// Modifiés ici et sur le serveur : à déverrouiller pour terminer la synchro.
    needs_unlock: Vec<String>,
    not_updated: Vec<Failure>,
    /// Coffres supprimés du serveur ; gardés sur cet appareil.
    removed: Vec<String>,
    /// Coffres communs dont le compte n'est plus membre ; supprimés de cet appareil.
    left: Vec<String>,
    not_left: Vec<Failure>,
    /// Coffres supprimés partout depuis un autre appareil, sans modification ici ; supprimés de
    /// cet appareil.
    deleted: Vec<String>,
    not_deleted: Vec<Failure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Renamed {
    name: String,
    new_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Downloaded {
    /// Nom sur le serveur.
    name: String,
    /// Nom sur cet appareil, différent si le premier était pris.
    local_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Failure {
    name: String,
    /// Message de l'erreur, en français.
    reason: String,
}

/// Coffre du serveur retiré de cet appareil, à télécharger de nouveau sur demande.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IgnoredVault {
    id: Uuid,
    name: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Disconnection {
    /// Le jeton n'a pas pu être révoqué : à révoquer depuis le panneau d'administration.
    revoke_error: Option<String>,
    /// Coffres retirés de cet appareil ; ils restent sur le serveur.
    removed: Vec<String>,
    kept: Vec<Kept>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct Kept {
    name: String,
    /// `None` : des modifications ne sont pas encore sur le serveur.
    reason: Option<String>,
}

impl AppState {
    /// Le serveur de l'appareil et l'état de chaque coffre, d'après la liste des coffres du
    /// serveur.
    pub fn server_status(&self) -> Result<ServerStatus, UiError> {
        // Serveur injoignable : les coffres restent utilisables, sans état.
        self.status(|| self.observe(self.vaults.vault_states()).unwrap_or_default())
    }

    /// État vu par l'interface ; `states` n'est demandé que si un serveur est enregistré.
    fn status(
        &self,
        states: impl FnOnce() -> Vec<(String, VaultState)>,
    ) -> Result<ServerStatus, UiError> {
        let address = self.vaults.server()?.map(|server| server.local_address);
        let config = self.vaults.config()?;
        let states: BTreeMap<String, VaultState> = match address {
            Some(_) => states(),
            None => Vec::new(),
        }
        .into_iter()
        .collect();
        let vaults = config
            .vaults
            .into_iter()
            .map(|(name, vault)| VaultStatus {
                state: states.get(&name).copied().map(state_name),
                name,
                shared: vault.shared,
            })
            .collect();
        // Un ancien serveur, oublié depuis, ne compte plus.
        let trouble = address.as_ref().and(self.trouble());
        Ok(ServerStatus {
            address,
            trouble,
            vaults,
            renamed: Vec::new(),
        })
    }

    /// Connecte l'appareil ; `account_name`, laissé vide sauf avec une clé administrateur,
    /// nomme le compte créé.
    pub fn connect(
        &self,
        address: &str,
        key: &Secret,
        device_name: &str,
        account_name: &str,
    ) -> Result<(), UiError> {
        let key = ConnectionKey::parse(key.expose())
            .map_err(|_| UiError::code("invalidConnectionKey"))?;
        let account_name = Some(account_name.trim()).filter(|name| !name.is_empty());
        let _one_at_a_time = self.exclusive();
        let connected = self
            .vaults
            .connect(address, &key, device_name.trim(), account_name);
        self.observe(connected).map_err(server_error)
    }

    /// Échange avec le serveur, sans mot de passe (bouton « Synchroniser » de la liste) : un
    /// coffre ouvert dont le compte n'est plus membre est fermé, il vient d'être supprimé.
    pub fn exchange(&self) -> Result<Exchange, UiError> {
        let _one_at_a_time = self.exclusive();
        self.exchange_now().map_err(server_error)
    }

    /// Échange automatique de la liste des coffres (SPEC §7), muet : `None` si un échange ou une
    /// synchro est déjà en cours. Renvoie l'état qui en résulte ; un serveur qui n'a pas répondu
    /// n'est pas recontacté pour le lire.
    pub fn refresh(&self) -> Result<Option<ServerStatus>, UiError> {
        let Some(one_at_a_time) = self.try_exclusive() else {
            return Ok(None);
        };
        let exchanged = self.vaults.server()?.map(|_| self.exchange_now());
        drop(one_at_a_time);
        let (status, renamed) = match exchanged {
            Some(Err(AppError::Port(_))) => (self.status(Vec::new), Vec::new()),
            Some(Ok(exchange)) => (self.server_status(), exchange.renamed),
            _ => (self.server_status(), Vec::new()),
        };
        status.map(|status| Some(ServerStatus { renamed, ..status }))
    }

    /// À appeler pendant [`AppState::exclusive`].
    fn exchange_now(&self) -> Result<Exchange, AppError> {
        self.exchanged(self.vaults.exchange())
    }

    /// Un coffre ouvert que l'échange vient de supprimer est fermé.
    fn exchanged(&self, done: Result<pswd_app::Exchange, AppError>) -> Result<Exchange, AppError> {
        let exchange = Exchange::from(self.observe(done)?);
        for name in exchange.left.iter().chain(&exchange.deleted) {
            self.close_without_publishing(name);
        }
        Ok(exchange)
    }

    /// Retire de cet appareil le coffre publié `name`, qui reste sur le serveur ; le nom est
    /// retapé pour confirmer. Le coffre ouvert n'est fermé qu'une fois retiré : refusé, il reste
    /// ouvert, à synchroniser.
    pub fn forget(&self, name: &str, confirmation: &str) -> Result<(), UiError> {
        confirm(name, confirmation)?;
        let _one_at_a_time = self.exclusive();
        let forgotten = self.vaults.forget(name);
        self.observe(forgotten).map_err(server_error)?;
        self.close_without_publishing(name);
        Ok(())
    }

    /// Coffres du serveur retirés de cet appareil.
    pub fn ignored_vaults(&self) -> Result<Vec<IgnoredVault>, UiError> {
        let ignored = self
            .observe(self.vaults.ignored_vaults())
            .map_err(server_error)?;
        Ok(ignored
            .into_iter()
            .map(|vault| IgnoredVault {
                id: vault.id,
                name: vault.name,
            })
            .collect())
    }

    /// Télécharge de nouveau le coffre `id`, retiré de cet appareil, par un échange.
    pub fn download_again(&self, id: Uuid) -> Result<Exchange, UiError> {
        let _one_at_a_time = self.exclusive();
        self.exchanged(self.vaults.download_again(id))
            .map_err(server_error)
    }

    /// Clé de connexion d'un appareil de plus pour le même compte, à montrer une seule fois.
    pub fn new_device_key(&self) -> Result<Secret, UiError> {
        self.vaults.new_device_key().map_err(server_error)
    }

    /// Sans `keep_local`, le coffre ouvert est d'abord verrouillé : il peut être retiré.
    pub fn disconnect(&self, keep_local: bool) -> Result<Disconnection, UiError> {
        if !keep_local {
            self.lock();
        }
        let _one_at_a_time = self.exclusive();
        let report = self.vaults.disconnect(keep_local)?;
        Ok(Disconnection {
            revoke_error: report.revoke_error.map(|error| error.to_string()),
            removed: report.removed,
            kept: report
                .kept
                .into_iter()
                .map(|(name, reason)| Kept {
                    name,
                    reason: match reason {
                        KeptReason::Unpublished => None,
                        KeptReason::Error(error) => Some(error.to_string()),
                    },
                })
                .collect(),
        })
    }
}

impl From<pswd_app::Exchange> for Exchange {
    fn from(done: pswd_app::Exchange) -> Self {
        let mut exchange = Self {
            renamed: done
                .renamed
                .into_iter()
                .map(|(name, new_name)| Renamed { name, new_name })
                .collect(),
            removed: done.removed,
            ..Self::default()
        };
        for (name, result) in done.downloaded {
            match result {
                Ok(local_name) => exchange.downloaded.push(Downloaded { name, local_name }),
                Err(error) => exchange.not_downloaded.push(failure(name, &error)),
            }
        }
        for (name, result) in done.published {
            match result {
                Ok(()) => exchange.published.push(name),
                Err(error) => exchange.not_published.push(failure(name, &error)),
            }
        }
        for (name, result) in done.updated {
            match result {
                Ok(Update::Sent) => exchange.sent.push(name),
                Ok(Update::Received) => exchange.received.push(name),
                Ok(Update::NeedsUnlock) => exchange.needs_unlock.push(name),
                Err(error) => exchange.not_updated.push(failure(name, &error)),
            }
        }
        for (name, result) in done.left {
            match result {
                Ok(()) => exchange.left.push(name),
                Err(error) => exchange.not_left.push(failure(name, &error)),
            }
        }
        for (name, result) in done.deleted {
            match result {
                Ok(()) => exchange.deleted.push(name),
                Err(error) => exchange.not_deleted.push(failure(name, &error)),
            }
        }
        exchange
    }
}

fn state_name(state: VaultState) -> &'static str {
    match state {
        VaultState::Local => "local",
        VaultState::Synced => "synced",
        VaultState::Outdated => "outdated",
        VaultState::NeedsUnlock => "needsUnlock",
        VaultState::Removed => "removed",
    }
}

fn failure(name: String, error: &AppError) -> Failure {
    Failure {
        name,
        reason: error.to_string(),
    }
}

/// Une erreur de port vient ici du serveur, pas d'un fichier.
pub(crate) fn server_error(error: AppError) -> UiError {
    match error {
        AppError::Port(error) if error.is_unreachable() => UiError::code("serverUnreachable"),
        AppError::Port(error) => UiError::new("serverError", error.to_string()),
        other => other.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::TcpListener;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicI64, Ordering};

    use pswd_app::Vaults;
    use pswd_core::crypto::KdfParams;
    use pswd_core::{Credentials, EntryContent, EntryKind, Timestamp};
    use pswd_infra::{ConfigFile, Locations, ServerFile};
    use pswd_server::testing::TestServer;
    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;
    use crate::testing::Silent;

    const PASSWORD: &str = "Maître-2026!";

    fn device(dir: &TempDir) -> AppState {
        device_with_clock(dir, || Timestamp::from_millis(1_000))
    }

    fn device_with_clock(dir: &TempDir, clock: fn() -> Timestamp) -> AppState {
        let vaults = Vaults::new(Locations::new(
            &dir.path().join("config"),
            &dir.path().join("data"),
        ));
        AppState::with_settings(vaults, Box::new(Silent), kdf(), clock, None, false, Silent)
    }

    fn kdf() -> KdfParams {
        KdfParams::new(8 * 1024, 1, 1).unwrap()
    }

    fn secret(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    /// Crée le coffre sans l'ouvrir : le verrouiller lancerait une synchro en arrière-plan,
    /// qui pourrait le publier à la place de l'échange vérifié par le test.
    fn create(state: &AppState, name: &str) {
        let now = (state.clock)();
        state
            .vaults
            .create(name, &secret(PASSWORD), kdf(), now)
            .unwrap();
    }

    fn connect_admin(state: &AppState) -> TestServer {
        let server = TestServer::start();
        state
            .connect(&server.address, &server.admin_key, " PC ", " Arthur ")
            .unwrap();
        server
    }

    fn connect_same_account(connected: &AppState, other: &AppState) {
        let address = connected.vaults.server().unwrap().unwrap().local_address;
        let key = connected.new_device_key().unwrap();
        other.connect(&address, &key, "Portable", "").unwrap();
    }

    fn sent<T: Serialize>(value: &T) -> Value {
        serde_json::to_value(value).unwrap()
    }

    fn status(state: &AppState) -> Value {
        sent(&state.server_status().unwrap())
    }

    /// Fait pointer `server.toml` vers une adresse qui coupe toute connexion.
    fn go_offline(dir: &TempDir) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for connection in listener.incoming() {
                drop(connection);
            }
        });
        point_to(dir, address);
    }

    fn point_to(dir: &TempDir, address: String) {
        let mut file = ServerFile::new(dir.path().join("data").join("server.toml"));
        let mut server = file.load().unwrap().unwrap();
        server.local_address = address;
        file.save(&server).unwrap();
    }

    /// Fichier du coffre `name`, lu dans la configuration de l'appareil.
    fn vault_file(dir: &Path, name: &str) -> PathBuf {
        ConfigFile::new(dir.join("config").join("config.toml"))
            .load()
            .unwrap()
            .vault(name)
            .unwrap()
            .vault_file()
    }

    #[test]
    fn status_shows_the_server_and_the_state_of_each_vault() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        create(&state, "perso");
        assert_eq!(
            status(&state),
            json!({
                "address": null,
                "trouble": null,
                "vaults": [{ "name": "perso", "state": null, "shared": false }],
            })
        );
        let server = connect_admin(&state);
        assert_eq!(status(&state)["vaults"][0]["state"], "local");
        state.exchange().unwrap();
        assert_eq!(status(&state)["address"], server.address.as_str());
        assert_eq!(status(&state)["vaults"][0]["state"], "synced");
        assert_eq!(state.disconnect(true), Ok(Disconnection::default()));
        assert_eq!(status(&state)["vaults"][0]["state"], Value::Null);
    }

    #[test]
    fn status_tells_since_when_the_registered_server_is_unreachable() {
        static NOW: AtomicI64 = AtomicI64::new(1_000);
        let at = |millis| NOW.store(millis, Ordering::Relaxed);
        let dir = TempDir::new().unwrap();
        let state = device_with_clock(&dir, || Timestamp::from_millis(NOW.load(Ordering::Relaxed)));
        let server = connect_admin(&state);
        assert_eq!(status(&state)["trouble"], Value::Null);

        go_offline(&dir);
        at(2_000);
        let unreachable = json!({ "since": 2_000, "reason": "serveur injoignable" });
        assert_eq!(status(&state)["trouble"], unreachable);
        at(3_000);
        assert!(state.exchange().is_err());
        assert!(state.account_status().is_err());
        assert_eq!(
            state.observe::<()>(Err(AppError::NotConnected)),
            Err(AppError::NotConnected)
        );
        assert_eq!(status(&state)["trouble"], unreachable);

        point_to(&dir, server.address.clone());
        assert_eq!(status(&state)["trouble"], Value::Null);
        go_offline(&dir);
        at(4_000);
        assert_eq!(status(&state)["trouble"]["since"], 4_000);
        state.disconnect(true).unwrap();
        assert_eq!(
            status(&state),
            json!({ "address": null, "trouble": null, "vaults": [] })
        );
    }

    #[test]
    fn refresh_exchanges_then_returns_the_status() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        create(&state, "perso");
        let local_only = sent(&state.refresh().unwrap().unwrap());
        assert_eq!(local_only["address"], Value::Null);
        assert_eq!(local_only["vaults"][0]["state"], Value::Null);

        let server = connect_admin(&state);
        let refreshed = sent(&state.refresh().unwrap().unwrap());
        assert_eq!(refreshed["address"], server.address.as_str());
        assert_eq!(refreshed["trouble"], Value::Null);
        assert_eq!(refreshed["vaults"][0]["state"], "synced");
        assert_eq!(refreshed.get("renamed"), None);
    }

    #[test]
    fn refresh_reports_a_vault_that_gave_its_name_away() {
        let (dir, other) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        let pc = device(&dir);
        create(&pc, "perso");
        connect_admin(&pc);
        pc.exchange().unwrap();
        let phone = device(&other);
        create(&phone, "perso");
        connect_same_account(&pc, &phone);
        let refreshed = sent(&phone.refresh().unwrap().unwrap());
        assert_eq!(
            refreshed["renamed"],
            json!([{ "name": "perso", "newName": "perso-2" }])
        );
        assert_eq!(refreshed["vaults"][1]["name"], "perso-2");
        assert_eq!(
            sent(&phone.refresh().unwrap().unwrap()).get("renamed"),
            None
        );
    }

    #[test]
    fn refresh_waits_for_nobody() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        connect_admin(&state);
        let busy = state.exclusive();
        assert_eq!(state.refresh(), Ok(None));
        drop(busy);
        assert!(state.refresh().unwrap().is_some());
    }

    #[test]
    fn refresh_still_runs_after_a_failed_exchange_panicked() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        std::thread::scope(|scope| {
            let crashed = scope.spawn(|| {
                let _held = state.exclusive();
                panic!("échange interrompu");
            });
            assert!(crashed.join().is_err());
        });
        assert!(state.refresh().unwrap().is_some());
    }

    #[test]
    fn refresh_reports_an_unreachable_server_without_asking_twice() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        create(&state, "perso");
        connect_admin(&state);
        go_offline(&dir);
        let offline = sent(&state.refresh().unwrap().unwrap());
        assert_eq!(offline["trouble"]["reason"], "serveur injoignable");
        assert_eq!(offline["vaults"][0]["state"], Value::Null);
    }

    #[test]
    fn refresh_reports_an_unreadable_configuration() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        connect_admin(&state);
        fs::create_dir_all(dir.path().join("config")).unwrap();
        fs::write(dir.path().join("config").join("config.toml"), "[[[").unwrap();
        assert_eq!(sent(&state.refresh().unwrap_err())["code"], "configInvalid");
    }

    #[test]
    fn connection_errors_are_explained() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        let server = TestServer::start();
        let connect =
            |key: &Secret, account: &str| state.connect(&server.address, key, "PC", account).err();
        assert_eq!(
            connect(&secret("pas une clé"), ""),
            Some(UiError::code("invalidConnectionKey"))
        );
        assert_eq!(
            connect(&server.admin_key, " "),
            Some(UiError::new(
                "serverError",
                "nom d'appareil ou de compte invalide"
            ))
        );
        assert_eq!(connect(&server.admin_key, "Arthur"), None);
        assert_eq!(
            connect(&server.admin_key, "Arthur"),
            Some(UiError::code("alreadyConnected"))
        );
    }

    #[test]
    fn a_disconnected_device_has_nothing_to_exchange() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        let not_connected = Some(UiError::code("notConnected"));
        assert_eq!(state.exchange().err(), not_connected);
        assert_eq!(state.new_device_key().err(), not_connected);
        assert_eq!(state.disconnect(true).err(), not_connected);
    }

    #[test]
    fn vaults_are_published_then_downloaded_by_another_device() {
        let dir = TempDir::new().unwrap();
        let pc = device(&dir);
        create(&pc, "perso");
        connect_admin(&pc);
        assert_eq!(
            sent(&pc.exchange().unwrap()),
            json!({
                "renamed": [],
                "downloaded": [],
                "notDownloaded": [],
                "published": ["perso"],
                "notPublished": [],
                "sent": [],
                "received": [],
                "needsUnlock": [],
                "notUpdated": [],
                "removed": [],
                "left": [],
                "notLeft": [],
                "deleted": [],
                "notDeleted": [],
            })
        );

        let other = TempDir::new().unwrap();
        let phone = device(&other);
        create(&phone, "perso");
        connect_same_account(&pc, &phone);
        let exchange = sent(&phone.exchange().unwrap());
        assert_eq!(
            exchange["renamed"],
            json!([{ "name": "perso", "newName": "perso-2" }])
        );
        assert_eq!(
            exchange["downloaded"],
            json!([{ "name": "perso", "localName": "perso" }])
        );
        assert_eq!(exchange["published"], json!(["perso-2"]));
    }

    #[test]
    fn failures_are_reported_per_vault() {
        let dir = TempDir::new().unwrap();
        let pc = device(&dir);
        create(&pc, "abime");
        connect_admin(&pc);
        fs::write(vault_file(dir.path(), "abime"), b"pas un coffre").unwrap();
        assert_eq!(
            sent(&pc.exchange().unwrap())["notPublished"],
            json!([{ "name": "abime", "reason": "réponse inattendue du serveur (400)" }])
        );

        create(&pc, "perso");
        pc.exchange().unwrap();
        let other = TempDir::new().unwrap();
        let phone = device(&other);
        connect_same_account(&pc, &phone);
        fs::write(other.path().join("data").join("vaults"), b"").unwrap();
        let exchange = sent(&phone.exchange().unwrap());
        assert_eq!(exchange["notDownloaded"][0]["name"], "perso");
        assert!(exchange["notDownloaded"][0]["reason"].is_string());
    }

    /// Ajoute une note au coffre `perso`, sans synchro.
    fn add_note(state: &AppState, title: &str) {
        state
            .vaults
            .open("perso", Credentials::Password(&secret(PASSWORD)))
            .unwrap()
            .edit(|vault| {
                vault.add(
                    EntryContent::new(EntryKind::Note, title.to_owned()),
                    Timestamp::from_millis(2),
                )
            })
            .unwrap();
    }

    #[test]
    fn vaults_are_updated_without_their_password() {
        let dir = TempDir::new().unwrap();
        let pc = device(&dir);
        create(&pc, "perso");
        connect_admin(&pc);
        pc.exchange().unwrap();
        let other = TempDir::new().unwrap();
        let phone = device(&other);
        connect_same_account(&pc, &phone);
        phone.exchange().unwrap();

        add_note(&phone, "Banque");
        assert_eq!(status(&phone)["vaults"][0]["state"], "outdated");
        assert_eq!(sent(&phone.exchange().unwrap())["sent"], json!(["perso"]));
        assert_eq!(sent(&pc.exchange().unwrap())["received"], json!(["perso"]));

        add_note(&pc, "Wi-Fi");
        add_note(&phone, "Cantine");
        pc.exchange().unwrap();
        assert_eq!(status(&phone)["vaults"][0]["state"], "needsUnlock");
        assert_eq!(
            sent(&phone.exchange().unwrap())["needsUnlock"],
            json!(["perso"])
        );
        fs::remove_file(vault_file(other.path(), "perso")).unwrap();
        assert_eq!(
            sent(&phone.exchange().unwrap())["notUpdated"][0]["name"],
            "perso"
        );
    }

    #[test]
    fn deleted_and_left_vaults_are_reported() {
        let dir = TempDir::new().unwrap();
        let pc = device(&dir);
        create(&pc, "perso");
        pc.vaults
            .create_shared(
                "famille",
                &secret(PASSWORD),
                kdf(),
                Timestamp::from_millis(0),
            )
            .unwrap();
        create(&pc, "travail");
        connect_admin(&pc);
        pc.exchange().unwrap();
        let other = TempDir::new().unwrap();
        let phone = device(&other);
        connect_same_account(&pc, &phone);
        phone.exchange().unwrap();
        phone.unlock("famille", &secret(PASSWORD)).unwrap();
        pc.vaults.delete_everywhere("perso").unwrap();
        let travail = pc
            .vaults
            .config()
            .unwrap()
            .vault("travail")
            .unwrap()
            .remote
            .clone();
        pc.vaults.delete_server_vault(travail.unwrap().id).unwrap();
        pc.vaults.leave("famille").unwrap();

        let exchange = sent(&phone.exchange().unwrap());
        assert_eq!(exchange["deleted"], json!(["perso"]));
        assert_eq!(exchange["removed"], json!(["travail"]));
        assert_eq!(exchange["left"], json!(["famille"]));
        assert_eq!(phone.current(), None);
        assert_eq!(status(&phone)["vaults"][0]["state"], "removed");
    }

    #[test]
    fn a_vault_deleted_everywhere_but_not_here_is_reported() {
        let dir = TempDir::new().unwrap();
        let pc = device(&dir);
        create(&pc, "perso");
        connect_admin(&pc);
        pc.exchange().unwrap();
        let other = TempDir::new().unwrap();
        let phone = device(&other);
        connect_same_account(&pc, &phone);
        phone.exchange().unwrap();
        pc.vaults.delete_everywhere("perso").unwrap();
        let mut temporary = vault_file(other.path(), "perso").into_os_string();
        temporary.push(".tmp");
        fs::create_dir(&temporary).unwrap();
        assert_eq!(
            sent(&phone.exchange().unwrap())["notDeleted"][0]["name"],
            "perso"
        );
    }

    #[test]
    fn a_vault_left_but_not_deleted_is_reported() {
        let dir = TempDir::new().unwrap();
        let pc = device(&dir);
        pc.vaults
            .create_shared(
                "famille",
                &secret(PASSWORD),
                kdf(),
                Timestamp::from_millis(0),
            )
            .unwrap();
        connect_admin(&pc);
        pc.exchange().unwrap();
        let other = TempDir::new().unwrap();
        let phone = device(&other);
        connect_same_account(&pc, &phone);
        phone.exchange().unwrap();
        pc.vaults.leave("famille").unwrap();
        let mut temporary = vault_file(other.path(), "famille").into_os_string();
        temporary.push(".tmp");
        fs::create_dir(&temporary).unwrap();
        assert_eq!(
            sent(&phone.exchange().unwrap())["notLeft"][0]["name"],
            "famille"
        );
    }

    #[test]
    fn an_offline_server_is_reported() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        create(&state, "perso");
        connect_admin(&state);
        go_offline(&dir);
        let offline = Some(UiError::code("serverUnreachable"));
        assert_eq!(state.exchange().err(), offline);
        assert_eq!(state.new_device_key().err(), offline);
        assert_eq!(status(&state)["vaults"][0]["state"], Value::Null);
    }

    #[test]
    fn disconnection_without_copies_removes_what_the_server_has() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        create(&state, "modifie");
        create(&state, "perso");
        connect_admin(&state);
        state.exchange().unwrap();
        state
            .vaults
            .open("modifie", Credentials::Password(&secret(PASSWORD)))
            .unwrap()
            .edit(|vault| {
                vault.add(
                    EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
                    Timestamp::from_millis(2),
                )
            })
            .unwrap();
        state.unlock("perso", &secret(PASSWORD)).unwrap();
        assert_eq!(
            sent(&state.disconnect(false).unwrap()),
            json!({
                "revokeError": null,
                "removed": ["perso"],
                "kept": [{ "name": "modifie", "reason": null }],
            })
        );
        assert_eq!(state.current(), None);
    }

    #[test]
    fn offline_disconnection_keeps_every_vault() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        create(&state, "perso");
        connect_admin(&state);
        state.exchange().unwrap();
        go_offline(&dir);
        assert_eq!(
            sent(&state.disconnect(false).unwrap()),
            json!({
                "revokeError": "serveur injoignable",
                "removed": [],
                "kept": [{ "name": "perso", "reason": "serveur injoignable" }],
            })
        );
    }

    #[test]
    fn a_vault_removed_from_the_device_comes_back_on_demand() {
        let dir = TempDir::new().unwrap();
        let state = device(&dir);
        create(&state, "perso");
        create(&state, "travail");
        connect_admin(&state);
        state.exchange().unwrap();
        add_note(&state, "Wi-Fi");
        state.unlock("travail", &secret(PASSWORD)).unwrap();
        let code = |error: UiError| sent(&error)["code"].clone();
        assert_eq!(
            code(state.forget("perso", "autre").unwrap_err()),
            "confirmationMismatch"
        );
        assert_eq!(
            code(state.forget("perso", "perso").unwrap_err()),
            "unpublishedChanges"
        );

        state.forget("travail", " travail ").unwrap();
        assert_eq!(state.current(), None);
        let names: Vec<String> = state.vaults.config().unwrap().vaults.into_keys().collect();
        assert_eq!(names, ["perso"]);
        let ignored = state.ignored_vaults().unwrap();
        assert_eq!(sent(&ignored)[0]["name"], "travail");
        let downloaded = sent(&state.download_again(ignored[0].id).unwrap());
        assert_eq!(downloaded["downloaded"][0]["localName"], "travail");
        assert_eq!(sent(&state.ignored_vaults().unwrap()), json!([]));

        go_offline(&dir);
        assert_eq!(
            code(state.ignored_vaults().unwrap_err()),
            "serverUnreachable"
        );
        assert_eq!(
            code(state.download_again(ignored[0].id).unwrap_err()),
            "serverUnreachable"
        );
    }
}
