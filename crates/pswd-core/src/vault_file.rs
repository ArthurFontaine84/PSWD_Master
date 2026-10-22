mod header;
mod payload;

use std::fmt;

use header::{Header, ParsedFile, PasswordEnvelope, RecoveryEnvelope};

use crate::crypto::{self, CryptoError, KdfParams, Key, Salt};
use crate::{RecoveryKey, Secret, Timestamp, Vault};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileError {
    Malformed,
    UnsupportedVersion(u16),
    UnknownEnvelope(u8),
    WrongPassword,
    WrongRecoveryKey,
    Corrupted,
    Internal,
    Crypto(CryptoError),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => f.write_str("fichier de coffre mal formé"),
            Self::UnsupportedVersion(version) => {
                write!(f, "version de format non prise en charge : {version}")
            }
            Self::UnknownEnvelope(kind) => write!(f, "type d'enveloppe inconnu : {kind}"),
            Self::WrongPassword => f.write_str("mot de passe maître incorrect"),
            Self::WrongRecoveryKey => f.write_str("clé de récupération incorrecte"),
            Self::Corrupted => f.write_str("coffre altéré ou corrompu"),
            Self::Internal => f.write_str("erreur interne de sérialisation du coffre"),
            Self::Crypto(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for FileError {}

impl From<CryptoError> for FileError {
    fn from(error: CryptoError) -> Self {
        Self::Crypto(error)
    }
}

/// Erreur de sérialisation JSON qui ne peut pas survenir : toutes les clés sont des chaînes et
/// l'écriture se fait en mémoire.
// Exclue de la couverture : aucun test ne peut l'atteindre.
#[cfg_attr(coverage_nightly, coverage(off))]
fn internal_error<E>(_: E) -> FileError {
    FileError::Internal
}

/// Points d'entrée de `cargo fuzz` : ils exposent les parseurs sans passer par Argon2.
#[cfg(fuzzing)]
pub mod fuzz {
    use super::{ParsedFile, payload};

    /// # Panics
    ///
    /// Si un en-tête accepté ne se réencode pas à l'identique : le fuzzer signale ainsi un bug.
    pub fn parse_file(data: &[u8]) {
        if let Ok(parsed) = ParsedFile::parse(data) {
            assert_eq!(parsed.header.encode(), parsed.header_bytes);
        }
    }

    /// # Panics
    ///
    /// Si un payload accepté ne se relit pas à l'identique après réécriture.
    pub fn decode_payload(data: &[u8]) {
        if let Ok(vault) = payload::decode(data) {
            match payload::encode(&vault) {
                Ok(json) => assert_eq!(payload::decode(&json).as_ref(), Ok(&vault)),
                Err(error) => panic!("réécriture impossible : {error}"),
            }
        }
    }
}

/// Coffre déverrouillé : garde la clé du coffre et l'en-tête, pour réécrire le fichier sans
/// redemander le mot de passe.
pub struct VaultFile {
    vault_key: Key,
    header: Header,
}

impl VaultFile {
    /// Prépare un nouveau coffre et renvoie sa clé de récupération, à montrer une seule fois.
    ///
    /// # Errors
    ///
    /// [`FileError::Crypto`] si le générateur aléatoire échoue.
    pub fn create(
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
    ) -> Result<(Self, RecoveryKey), FileError> {
        let vault_key = Key::generate()?;
        let recovery_key = RecoveryKey::generate()?;
        let header = Header {
            keys_updated_at: now,
            password: PasswordEnvelope::seal(&vault_key, password, params, Salt::generate()?)?,
            recovery: RecoveryEnvelope::seal(&vault_key, recovery_key.as_key())?,
        };
        Ok((Self { vault_key, header }, recovery_key))
    }

    /// # Errors
    ///
    /// [`FileError::WrongPassword`] si le mot de passe est incorrect, ou une erreur de format
    /// si le fichier est invalide.
    pub fn unlock_with_password(
        file: &[u8],
        password: &Secret,
    ) -> Result<(Self, Vault), FileError> {
        let parsed = ParsedFile::parse(file)?;
        let vault_key = parsed.header.password.open(password)?;
        Self::decrypt(parsed, vault_key)
    }

    /// # Errors
    ///
    /// [`FileError::WrongRecoveryKey`] si la clé est incorrecte, ou une erreur de format si le
    /// fichier est invalide.
    pub fn unlock_with_recovery_key(
        file: &[u8],
        recovery_key: &RecoveryKey,
    ) -> Result<(Self, Vault), FileError> {
        let parsed = ParsedFile::parse(file)?;
        let vault_key = parsed.header.recovery.open(recovery_key.as_key())?;
        Self::decrypt(parsed, vault_key)
    }

    fn decrypt(parsed: ParsedFile<'_>, vault_key: Key) -> Result<(Self, Vault), FileError> {
        let json = crypto::open(
            &vault_key,
            &parsed.payload_nonce,
            parsed.payload,
            parsed.header_bytes,
        )
        .map_err(|_| FileError::Corrupted)?;
        let vault = payload::decode(&json)?;
        let unlocked = Self {
            vault_key,
            header: parsed.header,
        };
        Ok((unlocked, vault))
    }

    /// Contenu complet du fichier `vault.enc` pour ce coffre.
    ///
    /// # Errors
    ///
    /// [`FileError::Crypto`] si le générateur aléatoire échoue.
    pub fn encrypt(&self, vault: &Vault) -> Result<Vec<u8>, FileError> {
        let mut file = self.header.encode();
        let json = payload::encode(vault)?;
        let sealed = crypto::seal(&self.vault_key, &json, &file)?;
        file.extend_from_slice(&sealed.nonce);
        file.extend_from_slice(&sealed.ciphertext);
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EntryContent, EntryKind};

    fn password(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    fn fast_params() -> KdfParams {
        KdfParams::new(8 * 1024, 1, 1).unwrap()
    }

    fn sample_vault() -> Vault {
        let mut vault = Vault::new();
        let content = EntryContent {
            password: Some(password("hunter2")),
            ..EntryContent::new(EntryKind::Login, "GitHub".to_owned())
        };
        vault.add(content, Timestamp::from_millis(1_000));
        vault
    }

    /// Fichier chiffré avec le mot de passe « maître », et sa clé de récupération.
    fn sample_file(vault: &Vault) -> (Vec<u8>, RecoveryKey) {
        let (unlocked, recovery_key) = VaultFile::create(
            &password("maître"),
            fast_params(),
            Timestamp::from_millis(500),
        )
        .unwrap();
        (unlocked.encrypt(vault).unwrap(), recovery_key)
    }

    #[test]
    fn password_unlocks_the_vault() {
        let vault = sample_vault();
        let (file, _) = sample_file(&vault);
        let (_, opened) = VaultFile::unlock_with_password(&file, &password("maître")).unwrap();
        assert_eq!(opened, vault);
    }

    #[test]
    fn recovery_key_unlocks_the_vault() {
        let vault = sample_vault();
        let (file, recovery_key) = sample_file(&vault);
        let (_, opened) = VaultFile::unlock_with_recovery_key(&file, &recovery_key).unwrap();
        assert_eq!(opened, vault);
    }

    #[test]
    fn displayed_recovery_key_unlocks_the_vault() {
        let vault = sample_vault();
        let (file, recovery_key) = sample_file(&vault);
        let typed = recovery_key
            .encode()
            .expose()
            .to_lowercase()
            .replace('-', " ");
        let parsed = RecoveryKey::parse(&typed).unwrap();
        let (_, opened) = VaultFile::unlock_with_recovery_key(&file, &parsed).unwrap();
        assert_eq!(opened, vault);
    }

    #[test]
    fn file_starts_with_magic_and_version() {
        let (file, _) = sample_file(&sample_vault());
        assert_eq!(file[..6], *b"PSWD\x01\x00");
    }

    #[test]
    fn password_is_never_stored_in_clear() {
        let (file, _) = sample_file(&sample_vault());
        for needle in [b"hunter2".as_slice(), "maître".as_bytes(), b"GitHub"] {
            assert!(!file.windows(needle.len()).any(|window| window == needle));
        }
    }

    #[test]
    fn unlocked_file_can_be_rewritten() {
        let mut vault = sample_vault();
        let (file, recovery_key) = sample_file(&vault);
        let (unlocked, _) = VaultFile::unlock_with_password(&file, &password("maître")).unwrap();
        vault.add(
            EntryContent::new(EntryKind::Note, "Wi-Fi".to_owned()),
            Timestamp::from_millis(2_000),
        );
        let rewritten = unlocked.encrypt(&vault).unwrap();
        let (_, opened) = VaultFile::unlock_with_recovery_key(&rewritten, &recovery_key).unwrap();
        assert_eq!(opened, vault);
    }

    #[test]
    fn each_write_uses_a_new_nonce() {
        let vault = sample_vault();
        let (file, _) = sample_file(&vault);
        let (unlocked, _) = VaultFile::unlock_with_password(&file, &password("maître")).unwrap();
        assert_ne!(unlocked.encrypt(&vault).unwrap(), file);
    }

    #[test]
    fn wrong_password_or_recovery_key_is_rejected() {
        let (file, recovery_key) = sample_file(&sample_vault());
        assert_eq!(
            VaultFile::unlock_with_password(&file, &password("maitre")).err(),
            Some(FileError::WrongPassword)
        );
        let other_key = RecoveryKey::generate().unwrap();
        assert_ne!(
            other_key.as_key().as_bytes(),
            recovery_key.as_key().as_bytes()
        );
        assert_eq!(
            VaultFile::unlock_with_recovery_key(&file, &other_key).err(),
            Some(FileError::WrongRecoveryKey)
        );
    }

    #[test]
    fn altered_header_or_payload_is_detected() {
        let (file, _) = sample_file(&sample_vault());
        // Octet 6 : `keys_updated_at` ; dernier octet : tag du payload.
        for position in [6, file.len() - 1] {
            let mut altered = file.clone();
            altered[position] ^= 1;
            assert_eq!(
                VaultFile::unlock_with_password(&altered, &password("maître")).err(),
                Some(FileError::Corrupted),
                "octet {position}"
            );
        }
    }

    #[test]
    fn invalid_file_is_rejected_before_any_key_derivation() {
        assert_eq!(
            VaultFile::unlock_with_password(b"", &password("maître")).err(),
            Some(FileError::Malformed)
        );
        assert_eq!(
            VaultFile::unlock_with_recovery_key(b"", &RecoveryKey::generate().unwrap()).err(),
            Some(FileError::Malformed)
        );
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (FileError::Malformed, "fichier de coffre mal formé"),
            (
                FileError::UnsupportedVersion(7),
                "version de format non prise en charge : 7",
            ),
            (
                FileError::UnknownEnvelope(3),
                "type d'enveloppe inconnu : 3",
            ),
            (FileError::WrongPassword, "mot de passe maître incorrect"),
            (
                FileError::WrongRecoveryKey,
                "clé de récupération incorrecte",
            ),
            (FileError::Corrupted, "coffre altéré ou corrompu"),
            (
                FileError::Internal,
                "erreur interne de sérialisation du coffre",
            ),
            (
                FileError::Crypto(CryptoError::InvalidKdfParams),
                "paramètres de dérivation de clé invalides",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
