use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use etcetera::BaseStrategy;
use pswd_core::ports::{LocalStore, PortError};
use pswd_core::{MAX_VAULT_NAME_LEN, is_valid_vault_name};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::FileStore;

const APP_DIR: &str = "pswd";
const DEV_APP_DIR: &str = "pswd-dev";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    NoHomeDirectory,
    File(PortError),
    Invalid(String),
    UnknownVault(String),
    VaultExists(String),
    InvalidVaultName(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHomeDirectory => f.write_str("dossier personnel introuvable"),
            Self::File(error) => write!(f, "configuration : {error}"),
            Self::Invalid(message) => write!(f, "configuration invalide : {message}"),
            Self::UnknownVault(name) => write!(f, "coffre inconnu : {name}"),
            Self::VaultExists(name) => write!(f, "un coffre nommé {name} existe déjà"),
            Self::InvalidVaultName(name) => write!(
                f,
                "nom de coffre invalide : « {name} » ({MAX_VAULT_NAME_LEN} caractères au plus, \
                 sans espace au début ni à la fin)"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Emplacements standard : configuration, serveur et dossiers des coffres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locations {
    config_file: PathBuf,
    server_file: PathBuf,
    vaults_dir: PathBuf,
}

impl Locations {
    /// Dossiers de l'utilisateur : `AppData\Roaming` sous Windows, `~/.config` et
    /// `~/.local/share` ailleurs.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NoHomeDirectory`] si le dossier personnel est introuvable.
    pub fn system() -> Result<Self, ConfigError> {
        Self::system_in(APP_DIR)
    }

    /// Mêmes dossiers de l'utilisateur, sous `pswd-dev` : une version de développement ne
    /// touche jamais les coffres de la version installée.
    ///
    /// # Errors
    ///
    /// [`ConfigError::NoHomeDirectory`] si le dossier personnel est introuvable.
    pub fn system_dev() -> Result<Self, ConfigError> {
        Self::system_in(DEV_APP_DIR)
    }

    fn system_in(app_dir: &str) -> Result<Self, ConfigError> {
        let base = etcetera::choose_base_strategy()
            .ok()
            .ok_or(ConfigError::NoHomeDirectory)?;
        Ok(Self::new(
            &base.config_dir().join(app_dir),
            &base.data_dir().join(app_dir),
        ))
    }

    #[must_use]
    pub fn new(config_dir: &Path, data_dir: &Path) -> Self {
        Self {
            config_file: config_dir.join("config.toml"),
            server_file: data_dir.join("server.toml"),
            vaults_dir: data_dir.join("vaults"),
        }
    }

    #[must_use]
    pub fn config_file(&self) -> &Path {
        &self.config_file
    }

    /// Serveur auquel l'appareil est connecté (`server.toml`).
    #[must_use]
    pub fn server_file(&self) -> &Path {
        &self.server_file
    }

    /// Dossier d'un coffre, désigné par son identifiant local : le nom du coffre est libre et
    /// peut changer, le dossier non.
    #[must_use]
    pub fn vault_dir(&self, id: Uuid) -> PathBuf {
        self.vaults_dir.join(id.simple().to_string())
    }
}

/// Un coffre : son dossier local et, s'il en a un, son exemplaire sur un serveur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultConfig {
    pub path: PathBuf,
    /// Coffre commun, à partager avec d'autres comptes du serveur.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub shared: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<RemoteLink>,
}

/// Lien d'un coffre avec son exemplaire sur un serveur, noté dès la réservation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteLink {
    /// Empreinte du certificat du serveur, en hexadécimal : reconnaît le serveur à la
    /// reconnexion.
    pub server: String,
    pub id: Uuid,
    /// Le coffre a été supprimé du serveur : il n'est plus synchronisé, sauf s'il est republié.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub removed: bool,
    /// Dernier état commun avec le serveur ; `None` tant qu'il n'est pas connu.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced: Option<SyncedState>,
}

/// État où la copie locale et celle du serveur avaient le même contenu : il dit, sans
/// déchiffrer le coffre, lequel des deux a changé depuis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyncedState {
    /// Version du serveur (`ETag`).
    pub version: String,
    /// Empreinte SHA-256 de la copie locale, en hexadécimal.
    pub file_hash: String,
}

impl VaultConfig {
    #[must_use]
    pub fn vault_file(&self) -> PathBuf {
        self.path.join("vault.enc")
    }
}

/// Thème de l'interface graphique ; `Auto` suit le réglage du système.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    Auto,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    /// Mobile : verrouillage après ce délai en arrière-plan ; 0 = immédiat.
    pub background_lock_seconds: u32,
    pub theme: Theme,
    /// Coffres du serveur retirés de cet appareil : ils n'y sont plus téléchargés.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ignored: Vec<Uuid>,
    pub vaults: BTreeMap<String, VaultConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 10,
            clipboard_clear_seconds: 20,
            background_lock_seconds: 60,
            theme: Theme::Auto,
            ignored: Vec::new(),
            vaults: BTreeMap::new(),
        }
    }
}

impl Config {
    /// # Errors
    ///
    /// [`ConfigError::UnknownVault`] si le coffre n'existe pas.
    pub fn vault(&self, name: &str) -> Result<&VaultConfig, ConfigError> {
        self.vaults
            .get(name)
            .ok_or_else(|| ConfigError::UnknownVault(name.to_owned()))
    }

    /// Vérifie qu'un nouveau coffre peut porter ce nom.
    ///
    /// # Errors
    ///
    /// [`ConfigError::InvalidVaultName`] si le nom est invalide (§7),
    /// [`ConfigError::VaultExists`] s'il est déjà pris.
    pub fn ensure_available(&self, name: &str) -> Result<(), ConfigError> {
        if !is_valid_vault_name(name) {
            return Err(ConfigError::InvalidVaultName(name.to_owned()));
        }
        if self.vaults.contains_key(name) {
            return Err(ConfigError::VaultExists(name.to_owned()));
        }
        Ok(())
    }

    /// Ajoute un coffre.
    ///
    /// # Errors
    ///
    /// Comme [`Config::ensure_available`].
    pub fn add_vault(&mut self, name: &str, vault: VaultConfig) -> Result<(), ConfigError> {
        self.ensure_available(name)?;
        self.vaults.insert(name.to_owned(), vault);
        Ok(())
    }

    /// Note le lien du coffre avec le serveur.
    ///
    /// # Errors
    ///
    /// [`ConfigError::UnknownVault`] si le coffre n'existe pas.
    pub fn link_vault(&mut self, name: &str, remote: RemoteLink) -> Result<(), ConfigError> {
        self.vaults
            .get_mut(name)
            .ok_or_else(|| ConfigError::UnknownVault(name.to_owned()))?
            .remote = Some(remote);
        Ok(())
    }

    /// Le coffre lié à l'exemplaire `id` d'un serveur.
    #[must_use]
    pub fn linked_to(&self, id: Uuid) -> Option<&str> {
        self.vaults
            .iter()
            .find(|(_, vault)| vault.remote.as_ref().is_some_and(|remote| remote.id == id))
            .map(|(name, _)| name.as_str())
    }

    /// Renomme un coffre : seul son nom change, son dossier et son lien avec le serveur restent.
    ///
    /// # Errors
    ///
    /// [`ConfigError::UnknownVault`] si le coffre n'existe pas, et comme
    /// [`Config::ensure_available`] pour le nouveau nom.
    pub fn rename_vault(&mut self, name: &str, new_name: &str) -> Result<(), ConfigError> {
        self.ensure_available(new_name)?;
        let vault = self.remove_vault(name)?;
        self.vaults.insert(new_name.to_owned(), vault);
        Ok(())
    }

    /// Retire un coffre : son nom redevient libre.
    ///
    /// # Errors
    ///
    /// [`ConfigError::UnknownVault`] si le coffre n'existe pas.
    pub fn remove_vault(&mut self, name: &str) -> Result<VaultConfig, ConfigError> {
        self.vaults
            .remove(name)
            .ok_or_else(|| ConfigError::UnknownVault(name.to_owned()))
    }

    #[must_use]
    pub fn auto_lock(&self) -> Duration {
        Duration::from_secs(u64::from(self.auto_lock_minutes) * 60)
    }

    #[must_use]
    pub fn clipboard_clear(&self) -> Duration {
        Duration::from_secs(u64::from(self.clipboard_clear_seconds))
    }

    #[must_use]
    pub fn background_lock(&self) -> Duration {
        Duration::from_secs(u64::from(self.background_lock_seconds))
    }
}

/// Fichier `config.toml`, écrit de façon atomique.
#[derive(Debug, Clone)]
pub struct ConfigFile {
    store: FileStore,
}

impl ConfigFile {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            store: FileStore::new(path),
        }
    }

    /// Fichier absent : configuration par défaut.
    ///
    /// # Errors
    ///
    /// Si le fichier existe mais ne peut pas être lu ou n'est pas une configuration valide.
    pub fn load(&self) -> Result<Config, ConfigError> {
        let Some(file) = self.store.load().map_err(ConfigError::File)? else {
            return Ok(Config::default());
        };
        let text = std::str::from_utf8(&file)
            .map_err(|_| ConfigError::Invalid("le fichier n'est pas en UTF-8".to_owned()))?;
        toml::from_str(text).map_err(|error| ConfigError::Invalid(error.message().to_owned()))
    }

    /// # Errors
    ///
    /// Si le fichier ne peut pas être écrit.
    pub fn save(&mut self, config: &Config) -> Result<(), ConfigError> {
        let text = toml::to_string(config).map_err(serialization_error)?;
        self.store
            .save_atomic(text.as_bytes())
            .map_err(ConfigError::File)
    }
}

/// Seul un chemin qui n'est pas de l'Unicode valide empêche d'écrire la configuration.
// Exclue de la couverture : un tel chemin ne se fabrique pas de la même façon sur chaque OS.
#[cfg_attr(coverage_nightly, coverage(off))]
// Prise par valeur pour s'écrire `map_err(serialization_error)`, sans closure.
#[allow(clippy::needless_pass_by_value)]
fn serialization_error(error: toml::ser::Error) -> ConfigError {
    ConfigError::Invalid(error.to_string())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn vault(path: &str) -> VaultConfig {
        VaultConfig {
            path: PathBuf::from(path),
            shared: false,
            remote: None,
        }
    }

    fn link(id: Uuid) -> RemoteLink {
        RemoteLink {
            server: "ab".repeat(32),
            id,
            removed: false,
            synced: None,
        }
    }

    fn config_file_in(dir: &TempDir) -> ConfigFile {
        ConfigFile::new(dir.path().join("config.toml"))
    }

    #[test]
    fn locations_follow_the_spec() {
        let locations = Locations::new(Path::new("conf"), Path::new("data"));
        assert_eq!(
            locations.config_file(),
            Path::new("conf").join("config.toml")
        );
        let id = Uuid::from_u128(0x1f);
        assert_eq!(
            locations.vault_dir(id),
            Path::new("data")
                .join("vaults")
                .join("0000000000000000000000000000001f")
        );
        assert_eq!(
            locations.server_file(),
            Path::new("data").join("server.toml")
        );
    }

    #[test]
    fn system_locations_are_inside_a_pswd_folder() {
        let locations = Locations::system().unwrap();
        assert!(
            locations
                .config_file()
                .ends_with(Path::new("pswd").join("config.toml"))
        );
        assert!(
            locations
                .vault_dir(Uuid::nil())
                .parent()
                .unwrap()
                .ends_with(Path::new("pswd").join("vaults"))
        );
    }

    #[test]
    fn dev_locations_are_beside_the_system_ones() {
        let system = Locations::system().unwrap();
        let dev = Locations::system_dev().unwrap();
        assert_eq!(
            dev.config_file(),
            system
                .config_file()
                .parent()
                .unwrap()
                .with_file_name("pswd-dev")
                .join("config.toml")
        );
        assert_eq!(
            dev.server_file(),
            system
                .server_file()
                .parent()
                .unwrap()
                .with_file_name("pswd-dev")
                .join("server.toml")
        );
    }

    #[test]
    fn missing_file_gives_the_defaults() {
        let dir = TempDir::new().unwrap();
        let config = config_file_in(&dir).load().unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.auto_lock(), Duration::from_secs(600));
        assert_eq!(config.clipboard_clear(), Duration::from_secs(20));
        assert_eq!(config.background_lock(), Duration::from_secs(60));
        assert_eq!(config.theme, Theme::Auto);
    }

    #[test]
    fn interface_settings_are_read_and_checked() {
        let dir = TempDir::new().unwrap();
        let file = config_file_in(&dir);
        std::fs::write(
            dir.path().join("config.toml"),
            "background_lock_seconds = 0
theme = \"dark\"
",
        )
        .unwrap();
        let config = file.load().unwrap();
        assert_eq!(config.background_lock_seconds, 0);
        assert_eq!(config.theme, Theme::Dark);
        std::fs::write(
            dir.path().join("config.toml"),
            "theme = \"sombre\"
",
        )
        .unwrap();
        assert!(matches!(file.load(), Err(ConfigError::Invalid(_))));
    }

    #[test]
    fn saved_config_is_loaded_back() {
        let dir = TempDir::new().unwrap();
        let mut file = config_file_in(&dir);
        let mut config = Config::default();
        config.add_vault("perso", vault("perso")).unwrap();
        config.add_vault("travail", vault("travail")).unwrap();
        config.add_vault("Été 2026", vault("ete")).unwrap();
        config.add_vault("a.b", vault("ab")).unwrap();
        config.clipboard_clear_seconds = 30;
        file.save(&config).unwrap();
        assert_eq!(file.load().unwrap(), config);
        let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(text.starts_with("auto_lock_minutes = 10\n"), "{text}");
        assert!(text.contains("[vaults.travail]"), "{text}");
        assert!(text.contains("[vaults.\"Été 2026\"]"), "{text}");
    }

    #[test]
    fn vault_links_are_saved_and_found() {
        let dir = TempDir::new().unwrap();
        let mut file = config_file_in(&dir);
        let mut config = Config::default();
        config.add_vault("perso", vault("perso")).unwrap();
        config.add_vault("travail", vault("travail")).unwrap();
        let id = Uuid::new_v4();
        config.link_vault("travail", link(id)).unwrap();
        file.save(&config).unwrap();

        let loaded = file.load().unwrap();
        assert_eq!(loaded, config);
        assert_eq!(loaded.linked_to(id), Some("travail"));
        assert_eq!(loaded.linked_to(Uuid::new_v4()), None);
        let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(text.contains("[vaults.travail.remote]"), "{text}");
        assert!(!text.contains("[vaults.perso.remote]"), "{text}");
        assert!(!text.contains("shared"), "{text}");
        assert!(!text.contains("removed"), "{text}");
        assert!(!text.contains("synced"), "{text}");
        assert_eq!(
            config.link_vault("inconnu", link(id)),
            Err(ConfigError::UnknownVault("inconnu".to_owned()))
        );
    }

    #[test]
    fn the_last_common_state_is_saved() {
        let dir = TempDir::new().unwrap();
        let mut file = config_file_in(&dir);
        let mut config = Config::default();
        let synced = SyncedState {
            version: "\"3\"".to_owned(),
            file_hash: "cd".repeat(32),
        };
        let perso = VaultConfig {
            remote: Some(RemoteLink {
                synced: Some(synced),
                ..link(Uuid::new_v4())
            }),
            ..vault("perso")
        };
        config.add_vault("perso", perso).unwrap();
        file.save(&config).unwrap();
        let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(text.contains("[vaults.perso.remote.synced]"), "{text}");
        assert_eq!(file.load().unwrap(), config);
    }

    #[test]
    fn shared_vaults_and_removed_links_are_saved() {
        let dir = TempDir::new().unwrap();
        let mut file = config_file_in(&dir);
        let mut config = Config::default();
        let famille = VaultConfig {
            shared: true,
            remote: Some(RemoteLink {
                removed: true,
                ..link(Uuid::new_v4())
            }),
            ..vault("famille")
        };
        config.add_vault("famille", famille).unwrap();
        file.save(&config).unwrap();
        let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(text.contains("shared = true"), "{text}");
        assert!(text.contains("removed = true"), "{text}");
        assert_eq!(file.load().unwrap(), config);
    }

    #[test]
    fn ignored_vaults_are_saved() {
        let dir = TempDir::new().unwrap();
        let mut file = config_file_in(&dir);
        let id = Uuid::new_v4();
        let config = Config {
            ignored: vec![id],
            ..Config::default()
        };
        file.save(&config).unwrap();
        let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(text.contains(&format!("ignored = [\"{id}\"]")), "{text}");
        assert_eq!(file.load().unwrap(), config);
    }

    #[test]
    fn missing_settings_take_their_default() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("config.toml"), "auto_lock_minutes = 5\n").unwrap();
        let config = config_file_in(&dir).load().unwrap();
        assert_eq!(config.auto_lock(), Duration::from_secs(300));
        assert_eq!(config.clipboard_clear_seconds, 20);
    }

    #[test]
    fn invalid_files_are_rejected() {
        let dir = TempDir::new().unwrap();
        let file = config_file_in(&dir);
        let cases: [&[u8]; 3] = [b"auto_lock_minutes = ", b"color = \"bleu\"\n", b"\xFF\n"];
        for content in cases {
            std::fs::write(dir.path().join("config.toml"), content).unwrap();
            assert!(
                matches!(file.load(), Err(ConfigError::Invalid(_))),
                "{content:?}"
            );
        }
    }

    #[test]
    fn unreadable_or_unwritable_file_is_an_error() {
        let dir = TempDir::new().unwrap();
        let mut file = ConfigFile::new(dir.path());
        assert!(matches!(file.load(), Err(ConfigError::File(_))));
        assert!(matches!(
            file.save(&Config::default()),
            Err(ConfigError::File(_))
        ));
    }

    #[test]
    fn vault_is_found_by_name() {
        let mut config = Config::default();
        config.add_vault("perso", vault("perso")).unwrap();
        config.add_vault("travail", vault("travail")).unwrap();
        assert_eq!(config.vault("perso"), Ok(&vault("perso")));
        assert_eq!(config.vault("travail"), Ok(&vault("travail")));
    }

    #[test]
    fn missing_vault_is_reported() {
        let config = Config::default();
        assert_eq!(
            config.vault("perso"),
            Err(ConfigError::UnknownVault("perso".to_owned()))
        );
    }

    #[test]
    fn vault_name_is_never_reused() {
        let mut config = Config::default();
        config.add_vault("perso", vault("a")).unwrap();
        assert_eq!(
            config.add_vault("perso", vault("b")),
            Err(ConfigError::VaultExists("perso".to_owned()))
        );
        assert_eq!(config.vault("perso"), Ok(&vault("a")));
    }

    #[test]
    fn removed_vault_frees_its_name() {
        let mut config = Config::default();
        config.add_vault("perso", vault("perso")).unwrap();
        config.add_vault("travail", vault("travail")).unwrap();
        assert_eq!(config.remove_vault("perso"), Ok(vault("perso")));
        assert_eq!(config.vaults.keys().collect::<Vec<_>>(), ["travail"]);
        assert_eq!(config.add_vault("perso", vault("autre")), Ok(()));
    }

    #[test]
    fn renamed_vault_keeps_its_folder_and_frees_its_name() {
        let mut config = Config::default();
        config.add_vault("banque", vault("dossier-a")).unwrap();
        config.add_vault("perso", vault("dossier-b")).unwrap();
        config.rename_vault("banque", "Banque 2").unwrap();
        assert_eq!(config.vault("Banque 2"), Ok(&vault("dossier-a")));
        assert_eq!(
            config.vaults.keys().collect::<Vec<_>>(),
            ["Banque 2", "perso"]
        );
        assert_eq!(
            config.rename_vault("perso", "Banque 2"),
            Err(ConfigError::VaultExists("Banque 2".to_owned()))
        );
        assert_eq!(
            config.rename_vault("perso", " perso"),
            Err(ConfigError::InvalidVaultName(" perso".to_owned()))
        );
        assert_eq!(
            config.rename_vault("inconnu", "autre"),
            Err(ConfigError::UnknownVault("inconnu".to_owned()))
        );
        assert_eq!(config.vault("perso"), Ok(&vault("dossier-b")));
    }

    #[test]
    fn removing_an_unknown_vault_is_an_error() {
        let mut config = Config::default();
        assert_eq!(
            config.remove_vault("perso"),
            Err(ConfigError::UnknownVault("perso".to_owned()))
        );
    }

    #[test]
    fn vault_names_are_checked() {
        let mut config = Config::default();
        let too_long = "a".repeat(MAX_VAULT_NAME_LEN + 1);
        for name in ["", " perso", "per\nso", &too_long] {
            assert_eq!(
                config.add_vault(name, vault("x")),
                Err(ConfigError::InvalidVaultName(name.to_owned())),
                "{name:?}"
            );
        }
        for name in ["Mon coffre", "Été", "Ete", "Banque", "banque"] {
            assert_eq!(config.add_vault(name, vault("x")), Ok(()), "{name:?}");
        }
    }

    #[test]
    fn vault_folder_holds_the_file() {
        let vault = vault("perso");
        assert_eq!(vault.vault_file(), Path::new("perso").join("vault.enc"));
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                ConfigError::NoHomeDirectory,
                "dossier personnel introuvable",
            ),
            (
                ConfigError::File(PortError::new("disque plein")),
                "configuration : disque plein",
            ),
            (
                ConfigError::Invalid("ligne 2".to_owned()),
                "configuration invalide : ligne 2",
            ),
            (
                ConfigError::UnknownVault("perso".to_owned()),
                "coffre inconnu : perso",
            ),
            (
                ConfigError::VaultExists("perso".to_owned()),
                "un coffre nommé perso existe déjà",
            ),
            (
                ConfigError::InvalidVaultName(" perso".to_owned()),
                "nom de coffre invalide : «  perso » (32 caractères au plus, sans espace au début \
                 ni à la fin)",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
