use std::fmt;

use zeroize::Zeroizing;

use crate::Secret;
use crate::crypto::{CryptoError, FINGERPRINT_LEN, KEY_LEN, fill_random};
use crate::key_text::{self, KeyTextError, SYMBOLS};

/// Partie secrète d'une clé de connexion, à usage unique.
pub const CONNECTION_SECRET_LEN: usize = 16;
/// Début de l'empreinte du certificat du serveur, porté par la clé.
pub const FINGERPRINT_PREFIX_LEN: usize = KEY_LEN - CONNECTION_SECRET_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionKeyError {
    InvalidCharacter,
    WrongLength,
    InvalidLastCharacter,
}

impl fmt::Display for ConnectionKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCharacter => f.write_str("caractère invalide dans la clé de connexion"),
            Self::WrongLength => write!(f, "la clé de connexion doit compter {SYMBOLS} caractères"),
            Self::InvalidLastCharacter => {
                f.write_str("dernier caractère de la clé de connexion invalide")
            }
        }
    }
}

impl std::error::Error for ConnectionKeyError {}

impl From<KeyTextError> for ConnectionKeyError {
    fn from(error: KeyTextError) -> Self {
        match error {
            KeyTextError::InvalidCharacter => Self::InvalidCharacter,
            KeyTextError::WrongLength => Self::WrongLength,
            KeyTextError::InvalidLastCharacter => Self::InvalidLastCharacter,
        }
    }
}

/// Clé de connexion d'un appareil à un serveur : un secret à usage unique, suivi du début de
/// l'empreinte du certificat du serveur. Même écriture que la clé de récupération.
pub struct ConnectionKey(Zeroizing<[u8; KEY_LEN]>);

impl ConnectionKey {
    /// Nouvelle clé pour le serveur dont le certificat a l'empreinte `fingerprint`.
    ///
    /// # Errors
    ///
    /// [`CryptoError::RandomUnavailable`] si le générateur aléatoire du système échoue.
    pub fn generate(fingerprint: &[u8; FINGERPRINT_LEN]) -> Result<Self, CryptoError> {
        let mut bytes = Zeroizing::new([0; KEY_LEN]);
        fill_random(&mut bytes[..CONNECTION_SECRET_LEN])?;
        bytes[CONNECTION_SECRET_LEN..].copy_from_slice(&fingerprint[..FINGERPRINT_PREFIX_LEN]);
        Ok(Self(bytes))
    }

    /// Texte à recopier dans l'application.
    #[must_use]
    pub fn encode(&self) -> Secret {
        key_text::encode(&self.0)
    }

    /// Lit une clé saisie par l'utilisateur, avec les mêmes tolérances que la clé de
    /// récupération.
    ///
    /// # Errors
    ///
    /// [`ConnectionKeyError`] si le texte ne peut pas être une clé de connexion.
    pub fn parse(text: &str) -> Result<Self, ConnectionKeyError> {
        Ok(Self(key_text::decode(text)?))
    }

    #[must_use]
    pub fn secret(&self) -> &[u8] {
        &self.0[..CONNECTION_SECRET_LEN]
    }

    /// Début de l'empreinte du certificat du serveur qui a créé la clé.
    #[must_use]
    pub fn fingerprint_prefix(&self) -> [u8; FINGERPRINT_PREFIX_LEN] {
        let mut prefix = [0; FINGERPRINT_PREFIX_LEN];
        prefix.copy_from_slice(&self.0[CONNECTION_SECRET_LEN..]);
        prefix
    }

    /// Le certificat d'empreinte `fingerprint` est-il celui du serveur qui a créé la clé ?
    #[must_use]
    pub fn matches(&self, fingerprint: &[u8; FINGERPRINT_LEN]) -> bool {
        fingerprint.starts_with(&self.fingerprint_prefix())
    }
}

impl fmt::Debug for ConnectionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConnectionKey(***)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fingerprint() -> [u8; FINGERPRINT_LEN] {
        std::array::from_fn(|i| u8::try_from(i).unwrap())
    }

    #[test]
    fn key_carries_a_secret_and_the_fingerprint_prefix() {
        let key = ConnectionKey::generate(&fingerprint()).unwrap();
        assert_eq!(key.secret().len(), CONNECTION_SECRET_LEN);
        assert_ne!(key.secret(), [0; CONNECTION_SECRET_LEN]);
        assert_eq!(
            key.fingerprint_prefix(),
            fingerprint()[..FINGERPRINT_PREFIX_LEN]
        );
    }

    #[test]
    fn secrets_differ_between_keys() {
        let first = ConnectionKey::generate(&fingerprint()).unwrap();
        let second = ConnectionKey::generate(&fingerprint()).unwrap();
        assert_ne!(first.secret(), second.secret());
    }

    #[test]
    fn only_the_announced_certificate_matches() {
        let key = ConnectionKey::generate(&fingerprint()).unwrap();
        assert!(key.matches(&fingerprint()));

        let mut other_end = fingerprint();
        other_end[FINGERPRINT_LEN - 1] ^= 1;
        assert!(key.matches(&other_end));

        let mut other_start = fingerprint();
        other_start[FINGERPRINT_PREFIX_LEN - 1] ^= 1;
        assert!(!key.matches(&other_start));
    }

    #[test]
    fn parse_reverses_encode() {
        let key = ConnectionKey::generate(&fingerprint()).unwrap();
        let parsed = ConnectionKey::parse(key.encode().expose()).unwrap();
        assert_eq!(*parsed.0, *key.0);
    }

    #[test]
    fn text_errors_become_connection_key_errors() {
        let valid = ConnectionKey::generate(&fingerprint()).unwrap().encode();
        let text = valid.expose();
        let cases = [
            ("U", ConnectionKeyError::InvalidCharacter),
            ("000G", ConnectionKeyError::WrongLength),
            (
                &format!("{}H", &text[..text.len() - 1]),
                ConnectionKeyError::InvalidLastCharacter,
            ),
        ];
        for (text, error) in cases {
            assert_eq!(ConnectionKey::parse(text).err(), Some(error), "{text}");
        }
    }

    #[test]
    fn debug_never_shows_the_key() {
        let key = ConnectionKey::generate(&fingerprint()).unwrap();
        assert_eq!(format!("{key:?}"), "ConnectionKey(***)");
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                ConnectionKeyError::InvalidCharacter,
                "caractère invalide dans la clé de connexion",
            ),
            (
                ConnectionKeyError::WrongLength,
                "la clé de connexion doit compter 52 caractères",
            ),
            (
                ConnectionKeyError::InvalidLastCharacter,
                "dernier caractère de la clé de connexion invalide",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
