mod header;
mod payload;

use std::fmt;

use header::{Header, ParsedFile, PasswordEnvelope, RecoveryEnvelope};

use crate::crypto::{self, CryptoError, KdfParams, Key, Salt};
use crate::{RecoveryKey, Secret, Timestamp, Vault, merge};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileError {
    Malformed,
    UnsupportedVersion(u16),
    UnknownEnvelope(u8),
    WrongPassword,
    WrongRecoveryKey,
    Corrupted,
    DifferentVault,
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
            Self::DifferentVault => f.write_str("le fichier distant appartient à un autre coffre"),
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

fn decrypt_payload(vault_key: &Key, parsed: &ParsedFile<'_>) -> Result<Vault, FileError> {
    let json = crypto::open(
        vault_key,
        &parsed.payload_nonce,
        parsed.payload,
        parsed.header_bytes,
    )
    .map_err(|_| FileError::Corrupted)?;
    payload::decode(&json)
}

/// Résultat de [`VaultFile::merge_remote`].
#[derive(Debug)]
pub struct RemoteMerge {
    pub vault: Vault,
    /// La copie locale doit être réécrite : entrées ou enveloppes reçues du distant.
    pub local_outdated: bool,
    /// Le distant doit être mis à jour : entrées ou enveloppes qu'il n'a pas.
    pub remote_outdated: bool,
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
        Self::unlocked(parsed, vault_key)
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
        Self::unlocked(parsed, vault_key)
    }

    /// Vérifie la structure du fichier sans le déchiffrer : aucun secret n'est nécessaire, mais
    /// une altération du contenu chiffré n'est pas détectée.
    ///
    /// # Errors
    ///
    /// Une erreur de format si le fichier n'est pas un coffre lisible par cette version.
    pub fn check_format(file: &[u8]) -> Result<(), FileError> {
        ParsedFile::parse(file).map(|_| ())
    }

    fn unlocked(parsed: ParsedFile<'_>, vault_key: Key) -> Result<(Self, Vault), FileError> {
        let vault = decrypt_payload(&vault_key, &parsed)?;
        let unlocked = Self {
            vault_key,
            header: parsed.header,
        };
        Ok((unlocked, vault))
    }

    /// Remplace le mot de passe maître : nouvelle enveloppe avec un nouveau sel. La clé du coffre
    /// et l'enveloppe de récupération ne changent pas.
    ///
    /// # Errors
    ///
    /// [`FileError::Crypto`] si le générateur aléatoire échoue ; le coffre reste alors inchangé.
    pub fn change_password(
        &mut self,
        password: &Secret,
        params: KdfParams,
        now: Timestamp,
    ) -> Result<(), FileError> {
        // EXTENSION(key-rotation): générer ici une nouvelle clé de coffre et refaire les deux
        // enveloppes.
        let salt = Salt::generate()?;
        self.header.password = PasswordEnvelope::seal(&self.vault_key, password, params, salt)?;
        // Toujours plus récent que l'en-tête actuel, même si l'horloge a reculé : la fusion doit
        // retenir ce changement.
        let after_current = self.header.keys_updated_at.as_millis().saturating_add(1);
        self.header.keys_updated_at = now.max(Timestamp::from_millis(after_current));
        Ok(())
    }

    /// Fusionne le fichier d'un autre appareil avec `local`.
    ///
    /// Les enveloppes retenues sont les plus récentes, ce qui propage un changement de mot de
    /// passe. Celles du fichier distant ne sont adoptées qu'après son authentification par la clé
    /// du coffre.
    ///
    /// # Errors
    ///
    /// [`FileError::Corrupted`] si le fichier distant est altéré ou chiffré par une autre clé,
    /// [`FileError::DifferentVault`] s'il contient un autre coffre, ou une erreur de format.
    pub fn merge_remote(
        &mut self,
        local: &Vault,
        remote_file: &[u8],
    ) -> Result<RemoteMerge, FileError> {
        let parsed = ParsedFile::parse(remote_file)?;
        let remote = decrypt_payload(&self.vault_key, &parsed)?;
        let vault = merge(local.clone(), remote.clone()).map_err(|_| FileError::DifferentVault)?;
        // À date égale, les octets de l'en-tête départagent : tous les appareils convergent.
        let current = self.header.encode();
        let adopt_remote_keys = (parsed.header.keys_updated_at, parsed.header_bytes)
            > (self.header.keys_updated_at, current.as_slice());
        let merge = RemoteMerge {
            local_outdated: adopt_remote_keys || vault != *local,
            remote_outdated: (!adopt_remote_keys && parsed.header_bytes != current.as_slice())
                || vault != remote,
            vault,
        };
        if adopt_remote_keys {
            self.header = parsed.header;
        }
        Ok(merge)
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
    fn format_is_checked_without_any_secret() {
        let (file, _) = sample_file(&sample_vault());
        assert_eq!(VaultFile::check_format(&file), Ok(()));
        assert_eq!(VaultFile::check_format(b""), Err(FileError::Malformed));
        let mut future = file.clone();
        future[4] = 2;
        assert_eq!(
            VaultFile::check_format(&future),
            Err(FileError::UnsupportedVersion(2))
        );
    }

    fn at(millis: i64) -> Timestamp {
        Timestamp::from_millis(millis)
    }

    fn note(title: &str) -> EntryContent {
        EntryContent::new(EntryKind::Note, title.to_owned())
    }

    fn titles(vault: &Vault) -> Vec<&str> {
        let mut titles: Vec<&str> = vault
            .entries()
            .map(|entry| entry.content().title.as_str())
            .collect();
        titles.sort_unstable();
        titles
    }

    /// Deux appareils qui ont ouvert le même fichier.
    fn two_devices() -> ((VaultFile, Vault), (VaultFile, Vault), RecoveryKey) {
        let (file, recovery_key) = sample_file(&sample_vault());
        let device = || VaultFile::unlock_with_password(&file, &password("maître")).unwrap();
        (device(), device(), recovery_key)
    }

    #[test]
    fn change_password_replaces_only_the_password() {
        let vault = sample_vault();
        let (file, recovery_key) = sample_file(&vault);
        let (mut unlocked, _) =
            VaultFile::unlock_with_password(&file, &password("maître")).unwrap();
        unlocked
            .change_password(&password("nouveau"), fast_params(), at(1_000))
            .unwrap();
        let file = unlocked.encrypt(&vault).unwrap();
        assert_eq!(
            VaultFile::unlock_with_password(&file, &password("maître")).err(),
            Some(FileError::WrongPassword)
        );
        let (_, opened) = VaultFile::unlock_with_password(&file, &password("nouveau")).unwrap();
        assert_eq!(opened, vault);
        let (_, opened) = VaultFile::unlock_with_recovery_key(&file, &recovery_key).unwrap();
        assert_eq!(opened, vault);
    }

    #[test]
    fn recovery_key_allows_choosing_a_new_password() {
        let vault = sample_vault();
        let (file, recovery_key) = sample_file(&vault);
        let (mut unlocked, opened) =
            VaultFile::unlock_with_recovery_key(&file, &recovery_key).unwrap();
        unlocked
            .change_password(&password("retrouvé"), fast_params(), at(1_000))
            .unwrap();
        let file = unlocked.encrypt(&opened).unwrap();
        let (_, reopened) = VaultFile::unlock_with_password(&file, &password("retrouvé")).unwrap();
        assert_eq!(reopened, vault);
    }

    #[test]
    fn keys_date_moves_forward_even_if_the_clock_goes_back() {
        let ((mut unlocked, _), _, _) = two_devices();
        unlocked
            .change_password(&password("a"), fast_params(), at(2_000))
            .unwrap();
        assert_eq!(unlocked.header.keys_updated_at, at(2_000));
        unlocked
            .change_password(&password("b"), fast_params(), at(100))
            .unwrap();
        assert_eq!(unlocked.header.keys_updated_at, at(2_001));
    }

    #[test]
    fn merge_remote_unites_entries_from_both_devices() {
        let ((mut a, mut vault_a), (b, mut vault_b), _) = two_devices();
        vault_a.add(note("de A"), at(2_000));
        vault_b.add(note("de B"), at(2_000));
        let merged = a
            .merge_remote(&vault_a, &b.encrypt(&vault_b).unwrap())
            .unwrap();
        assert_eq!(titles(&merged.vault), ["GitHub", "de A", "de B"]);
        assert!(merged.local_outdated);
        assert!(merged.remote_outdated);
    }

    #[test]
    fn identical_files_need_no_update() {
        let ((mut a, vault), (b, _), _) = two_devices();
        let merged = a.merge_remote(&vault, &b.encrypt(&vault).unwrap()).unwrap();
        assert_eq!(merged.vault, vault);
        assert!(!merged.local_outdated);
        assert!(!merged.remote_outdated);
    }

    #[test]
    fn entry_missing_on_one_side_only_updates_that_side() {
        let ((mut a, vault), (b, mut vault_b), _) = two_devices();
        vault_b.add(note("de B"), at(2_000));
        let file_b = b.encrypt(&vault_b).unwrap();
        let merged = a.merge_remote(&vault, &file_b).unwrap();
        assert!(merged.local_outdated);
        assert!(!merged.remote_outdated);
        let merged = a
            .merge_remote(&vault_b, &b.encrypt(&vault).unwrap())
            .unwrap();
        assert!(!merged.local_outdated);
        assert!(merged.remote_outdated);
    }

    #[test]
    fn newer_remote_password_is_adopted() {
        let ((mut a, vault), (mut b, _), _) = two_devices();
        b.change_password(&password("nouveau"), fast_params(), at(2_000))
            .unwrap();
        let merged = a.merge_remote(&vault, &b.encrypt(&vault).unwrap()).unwrap();
        assert!(merged.local_outdated);
        assert!(!merged.remote_outdated);
        let file = a.encrypt(&merged.vault).unwrap();
        assert_eq!(
            VaultFile::unlock_with_password(&file, &password("maître")).err(),
            Some(FileError::WrongPassword)
        );
        assert!(VaultFile::unlock_with_password(&file, &password("nouveau")).is_ok());
    }

    #[test]
    fn newer_local_password_is_kept() {
        let ((mut a, vault), (b, _), _) = two_devices();
        a.change_password(&password("nouveau"), fast_params(), at(2_000))
            .unwrap();
        let merged = a.merge_remote(&vault, &b.encrypt(&vault).unwrap()).unwrap();
        assert!(!merged.local_outdated);
        assert!(merged.remote_outdated);
        let file = a.encrypt(&merged.vault).unwrap();
        assert!(VaultFile::unlock_with_password(&file, &password("nouveau")).is_ok());
    }

    #[test]
    fn simultaneous_password_changes_converge() {
        let ((mut a, vault), (mut b, _), _) = two_devices();
        a.change_password(&password("pour A"), fast_params(), at(2_000))
            .unwrap();
        b.change_password(&password("pour B"), fast_params(), at(2_000))
            .unwrap();
        let file_a = a.encrypt(&vault).unwrap();
        let file_b = b.encrypt(&vault).unwrap();
        a.merge_remote(&vault, &file_b).unwrap();
        b.merge_remote(&vault, &file_a).unwrap();
        assert_eq!(a.header, b.header);
    }

    #[test]
    fn file_of_another_vault_is_rejected() {
        let ((mut a, vault), _, _) = two_devices();
        let (other, _) = sample_file(&vault);
        assert_eq!(
            a.merge_remote(&vault, &other).err(),
            Some(FileError::Corrupted)
        );
        assert_eq!(
            a.merge_remote(&vault, b"").err(),
            Some(FileError::Malformed)
        );
    }

    #[test]
    fn same_key_but_different_vault_id_is_rejected() {
        let ((mut a, vault), _, _) = two_devices();
        let foreign = a.encrypt(&Vault::new()).unwrap();
        assert_eq!(
            a.merge_remote(&vault, &foreign).err(),
            Some(FileError::DifferentVault)
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
                FileError::DifferentVault,
                "le fichier distant appartient à un autre coffre",
            ),
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
