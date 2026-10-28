//! `server.toml` : le serveur auquel l'appareil est connecté. Son existence signifie
//! « connecté ».

use std::fmt::Write as _;
use std::path::PathBuf;

use pswd_core::Secret;
use pswd_core::crypto::FINGERPRINT_LEN;
use pswd_core::ports::{LocalStore, PortError};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{ConfigError, FileStore, ServerClient};

/// Ce que l'appareil retient de son serveur.
pub struct ServerConfig {
    pub local_address: String,
    pub public_address: Option<String>,
    pub fingerprint: [u8; FINGERPRINT_LEN],
    pub token: Secret,
}

impl ServerConfig {
    /// Adresses à essayer, dans l'ordre : locale, puis publique.
    #[must_use]
    pub fn addresses(&self) -> Vec<String> {
        std::iter::once(self.local_address.clone())
            .chain(self.public_address.clone())
            .collect()
    }

    /// Empreinte du certificat en hexadécimal, telle que notée dans le lien d'un coffre.
    #[must_use]
    pub fn server_id(&self) -> String {
        self.fingerprint.iter().fold(
            String::with_capacity(FINGERPRINT_LEN * 2),
            |mut text, byte| {
                let _ = write!(text, "{byte:02x}");
                text
            },
        )
    }

    /// Client de ce serveur.
    ///
    /// # Errors
    ///
    /// Si le client HTTPS ne peut pas être créé.
    pub fn client(&self) -> Result<ServerClient, PortError> {
        ServerClient::new(self.addresses(), self.fingerprint, self.token.clone())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    local_address: String,
    public_address: Option<String>,
    fingerprint: String,
    token: String,
}

#[derive(Serialize)]
struct ToStore<'a> {
    local_address: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    public_address: Option<&'a str>,
    fingerprint: String,
    token: &'a str,
}

/// Fichier `server.toml`, lisible par le seul utilisateur, écrit de façon atomique.
#[derive(Debug, Clone)]
pub struct ServerFile {
    store: FileStore,
}

impl ServerFile {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            store: FileStore::private(path),
        }
    }

    /// `None` si l'appareil n'est connecté à aucun serveur.
    ///
    /// # Errors
    ///
    /// Si le fichier existe mais ne peut pas être lu ou n'est pas valide.
    pub fn load(&self) -> Result<Option<ServerConfig>, ConfigError> {
        let Some(file) = self.store.load().map_err(ConfigError::File)? else {
            return Ok(None);
        };
        let file = Zeroizing::new(file);
        let text = std::str::from_utf8(&file).map_err(|_| invalid("pas en UTF-8"))?;
        let stored: Stored = toml::from_str(text).map_err(|_| invalid("contenu illisible"))?;
        let token = Secret::new(stored.token);
        let fingerprint =
            parse_fingerprint(&stored.fingerprint).ok_or_else(|| invalid("empreinte illisible"))?;
        Ok(Some(ServerConfig {
            local_address: stored.local_address,
            public_address: stored.public_address,
            fingerprint,
            token,
        }))
    }

    /// # Errors
    ///
    /// Si le fichier ne peut pas être écrit.
    pub fn save(&mut self, server: &ServerConfig) -> Result<(), ConfigError> {
        let stored = ToStore {
            local_address: &server.local_address,
            public_address: server.public_address.as_deref(),
            fingerprint: server.server_id(),
            token: server.token.expose(),
        };
        let text = Zeroizing::new(toml::to_string(&stored).map_err(serialization_error)?);
        self.store
            .save_atomic(text.as_bytes())
            .map_err(ConfigError::File)
    }

    /// Un fichier déjà absent n'est pas une erreur.
    ///
    /// # Errors
    ///
    /// Si le fichier existe mais ne peut pas être supprimé.
    pub fn remove(&self) -> Result<(), ConfigError> {
        self.store.remove().map_err(ConfigError::File)
    }
}

/// Les messages ne reprennent jamais le détail de `toml`, qui peut citer le jeton.
fn invalid(problem: &str) -> ConfigError {
    ConfigError::Invalid(format!("server.toml : {problem}"))
}

/// Seule une adresse qui n'est pas de l'Unicode valide empêche d'écrire le fichier, et une
/// `String` en est toujours.
// Exclue de la couverture : aucun test ne peut l'atteindre.
#[cfg_attr(coverage_nightly, coverage(off))]
#[allow(clippy::needless_pass_by_value)]
fn serialization_error(error: toml::ser::Error) -> ConfigError {
    ConfigError::Invalid(error.to_string())
}

fn parse_fingerprint(text: &str) -> Option<[u8; FINGERPRINT_LEN]> {
    if text.len() != FINGERPRINT_LEN * 2 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut fingerprint = [0; FINGERPRINT_LEN];
    for (index, byte) in fingerprint.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * index..2 * index + 2], 16).ok()?;
    }
    Some(fingerprint)
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn server() -> ServerConfig {
        let mut fingerprint = [0; FINGERPRINT_LEN];
        fingerprint[0] = 0xAB;
        fingerprint[31] = 0x01;
        ServerConfig {
            local_address: "192.168.1.10:8443".to_owned(),
            public_address: None,
            fingerprint,
            token: Secret::new("0f".repeat(32)),
        }
    }

    fn file_in(dir: &TempDir) -> ServerFile {
        ServerFile::new(dir.path().join("server.toml"))
    }

    fn write(dir: &TempDir, text: &str) {
        std::fs::write(dir.path().join("server.toml"), text).unwrap();
    }

    #[test]
    fn missing_file_means_not_connected() {
        let dir = TempDir::new().unwrap();
        assert!(file_in(&dir).load().unwrap().is_none());
    }

    #[test]
    fn saved_server_is_loaded_back() {
        let dir = TempDir::new().unwrap();
        let mut file = file_in(&dir);
        file.save(&server()).unwrap();
        let text = std::fs::read_to_string(dir.path().join("server.toml")).unwrap();
        assert_eq!(
            text,
            format!(
                "local_address = \"192.168.1.10:8443\"\nfingerprint = \"ab{}01\"\ntoken = \"{}\"\n",
                "00".repeat(30),
                "0f".repeat(32)
            )
        );

        let loaded = file.load().unwrap().unwrap();
        assert_eq!(loaded.local_address, "192.168.1.10:8443");
        assert_eq!(loaded.public_address, None);
        assert_eq!(loaded.fingerprint, server().fingerprint);
        assert_eq!(loaded.token, server().token);
        assert_eq!(loaded.addresses(), ["192.168.1.10:8443"]);
    }

    #[test]
    fn public_address_is_tried_after_the_local_one() {
        let dir = TempDir::new().unwrap();
        let mut file = file_in(&dir);
        let mut server = server();
        server.public_address = Some("pswd.example.org:8443".to_owned());
        file.save(&server).unwrap();
        assert_eq!(
            file.load().unwrap().unwrap().addresses(),
            ["192.168.1.10:8443", "pswd.example.org:8443"]
        );
    }

    #[test]
    fn server_id_is_the_fingerprint_in_hexadecimal() {
        assert_eq!(server().server_id(), format!("ab{}01", "00".repeat(30)));
    }

    #[test]
    fn invalid_files_are_rejected_without_quoting_them() {
        let dir = TempDir::new().unwrap();
        let file = file_in(&dir);
        let token = "0f".repeat(32);
        let fingerprint = "ab".repeat(32);
        let cases = [
            (
                format!("local_address = \"a:1\"\nfingerprint = \"{fingerprint}\"\n"),
                "contenu illisible",
            ),
            (
                format!("token = \"{token}\" fingerprint"),
                "contenu illisible",
            ),
            (
                format!("local_address = \"a:1\"\nfingerprint = \"abc\"\ntoken = \"{token}\"\n"),
                "empreinte illisible",
            ),
            (
                format!(
                    "local_address = \"a:1\"\nfingerprint = \"{}\"\ntoken = \"{token}\"\n",
                    "zz".repeat(32)
                ),
                "empreinte illisible",
            ),
            (
                format!(
                    "local_address = \"a:1\"\nfingerprint = \"+f{}\"\ntoken = \"{token}\"\n",
                    "a".repeat(62)
                ),
                "empreinte illisible",
            ),
        ];
        for (text, problem) in cases {
            write(&dir, &text);
            let error = file.load().err().unwrap();
            assert_eq!(error, invalid(problem), "{text}");
            assert!(!error.to_string().contains(&token));
        }
        std::fs::write(dir.path().join("server.toml"), b"\xFF").unwrap();
        assert_eq!(file.load().err(), Some(invalid("pas en UTF-8")));
    }

    #[test]
    fn removed_file_means_not_connected() {
        let dir = TempDir::new().unwrap();
        let mut file = file_in(&dir);
        file.save(&server()).unwrap();
        file.remove().unwrap();
        assert!(file.load().unwrap().is_none());
        assert_eq!(file.remove(), Ok(()));
    }

    #[test]
    fn unreadable_or_unwritable_file_is_an_error() {
        let dir = TempDir::new().unwrap();
        let mut file = ServerFile::new(dir.path());
        assert!(matches!(file.load(), Err(ConfigError::File(_))));
        assert!(matches!(file.save(&server()), Err(ConfigError::File(_))));
        assert!(matches!(file.remove(), Err(ConfigError::File(_))));
    }

    #[test]
    fn client_targets_every_address() {
        let mut server = server();
        server.public_address = Some("pswd.example.org:8443".to_owned());
        assert!(server.client().is_ok());
    }
}
