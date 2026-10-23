use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use etcetera::BaseStrategy;
use pswd_core::ports::{LocalStore, PortError};
use serde::{Deserialize, Serialize};

use crate::FileStore;

const APP_DIR: &str = "pswd";
const MAX_VAULT_NAME_LEN: usize = 64;
/// Noms de fichiers interdits par Windows, quelle que soit la casse.
const RESERVED_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

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
                "nom de coffre invalide : « {name} » (lettres, chiffres, - et _ ; \
                 {MAX_VAULT_NAME_LEN} caractères au plus)"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Emplacements standard : configuration et dossiers des coffres.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locations {
    config_file: PathBuf,
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
        let base = etcetera::choose_base_strategy()
            .ok()
            .ok_or(ConfigError::NoHomeDirectory)?;
        Ok(Self::new(
            &base.config_dir().join(APP_DIR),
            &base.data_dir().join(APP_DIR),
        ))
    }

    #[must_use]
    pub fn new(config_dir: &Path, data_dir: &Path) -> Self {
        Self {
            config_file: config_dir.join("config.toml"),
            vaults_dir: data_dir.join("vaults"),
        }
    }

    #[must_use]
    pub fn config_file(&self) -> &Path {
        &self.config_file
    }

    /// Dossier proposé pour un nouveau coffre.
    #[must_use]
    pub fn vault_dir(&self, name: &str) -> PathBuf {
        self.vaults_dir.join(name)
    }
}

/// Un coffre : son dossier local.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VaultConfig {
    pub path: PathBuf,
}

impl VaultConfig {
    #[must_use]
    pub fn vault_file(&self) -> PathBuf {
        self.path.join("vault.enc")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub auto_lock_minutes: u32,
    pub clipboard_clear_seconds: u32,
    pub vaults: BTreeMap<String, VaultConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 10,
            clipboard_clear_seconds: 20,
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
    /// [`ConfigError::InvalidVaultName`] si le nom ne peut pas servir de nom de dossier,
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
}

/// Le nom sert de nom de dossier : ni séparateur, ni `..`, ni
/// nom réservé par Windows.
fn is_valid_vault_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_VAULT_NAME_LEN
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !RESERVED_NAMES.contains(&name.to_ascii_lowercase().as_str())
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
        assert_eq!(
            locations.vault_dir("perso"),
            Path::new("data").join("vaults").join("perso")
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
                .vault_dir("perso")
                .ends_with(Path::new("pswd").join("vaults").join("perso"))
        );
    }

    #[test]
    fn missing_file_gives_the_defaults() {
        let dir = TempDir::new().unwrap();
        let config = config_file_in(&dir).load().unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.auto_lock(), Duration::from_secs(600));
        assert_eq!(config.clipboard_clear(), Duration::from_secs(20));
    }

    #[test]
    fn saved_config_is_loaded_back() {
        let dir = TempDir::new().unwrap();
        let mut file = config_file_in(&dir);
        let mut config = Config::default();
        config.add_vault("perso", vault("perso")).unwrap();
        config.add_vault("travail", vault("travail")).unwrap();
        config.clipboard_clear_seconds = 30;
        file.save(&config).unwrap();
        assert_eq!(file.load().unwrap(), config);
        let text = std::fs::read_to_string(dir.path().join("config.toml")).unwrap();
        assert!(text.starts_with("auto_lock_minutes = 10\n"), "{text}");
        assert!(text.contains("[vaults.travail]"), "{text}");
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
    fn removing_an_unknown_vault_is_an_error() {
        let mut config = Config::default();
        assert_eq!(
            config.remove_vault("perso"),
            Err(ConfigError::UnknownVault("perso".to_owned()))
        );
    }

    #[test]
    fn vault_names_must_be_safe_folder_names() {
        let mut config = Config::default();
        let too_long = "a".repeat(MAX_VAULT_NAME_LEN + 1);
        for name in [
            "",
            "a/b",
            "a\\b",
            "..",
            "coffre perso",
            "é",
            "CON",
            "lpt1",
            &too_long,
        ] {
            assert_eq!(
                config.add_vault(name, vault("x")),
                Err(ConfigError::InvalidVaultName(name.to_owned())),
                "{name:?}"
            );
        }
        let longest = "a".repeat(MAX_VAULT_NAME_LEN);
        for name in ["Perso-2_x", "console", &longest] {
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
                ConfigError::InvalidVaultName("a/b".to_owned()),
                "nom de coffre invalide : « a/b » (lettres, chiffres, - et _ ; 64 caractères au plus)",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
