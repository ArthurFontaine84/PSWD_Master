use std::fmt;

use crate::Secret;
use crate::crypto::{CryptoError, Key};
use crate::key_text::{self, GROUP_LEN, GROUPS, KeyTextError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryKeyError {
    /// Pas sous la forme `XXXX-XXXX-…` (13 groupes de 4 caractères séparés par des tirets).
    Malformed,
    /// Bien formée, mais aucune clé ne s'écrit ainsi.
    InvalidLastCharacter,
}

impl fmt::Display for RecoveryKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Malformed => write!(
                f,
                "clé de récupération mal saisie : {GROUPS} groupes de {GROUP_LEN} caractères séparés par des tirets"
            ),
            Self::InvalidLastCharacter => {
                f.write_str("dernier caractère de la clé de récupération invalide")
            }
        }
    }
}

impl std::error::Error for RecoveryKeyError {}

impl From<KeyTextError> for RecoveryKeyError {
    fn from(error: KeyTextError) -> Self {
        match error {
            KeyTextError::Malformed => Self::Malformed,
            KeyTextError::InvalidLastCharacter => Self::InvalidLastCharacter,
        }
    }
}

/// Clé de récupération : 32 octets aléatoires, montrés une seule fois à l'utilisateur sous la
/// forme `XXXX-XXXX-…` (13 groupes de 4 caractères).
#[derive(Clone)]
pub struct RecoveryKey(Key);

impl RecoveryKey {
    /// # Errors
    ///
    /// [`CryptoError::RandomUnavailable`] si le générateur aléatoire du système échoue.
    pub fn generate() -> Result<Self, CryptoError> {
        Key::generate().map(Self)
    }

    /// Texte à montrer à l'utilisateur.
    #[must_use]
    pub fn encode(&self) -> Secret {
        key_text::encode(self.0.as_bytes())
    }

    /// Lit une clé saisie par l'utilisateur : l'écriture montrée, tirets compris, en majuscules
    /// ou en minuscules ; les espaces autour sont ignorés.
    ///
    /// # Errors
    ///
    /// [`RecoveryKeyError`] si le texte ne peut pas être une clé de récupération.
    pub fn parse(text: &str) -> Result<Self, RecoveryKeyError> {
        Ok(Self(Key::from_bytes(key_text::decode(text)?)))
    }

    pub(crate) fn as_key(&self) -> &Key {
        &self.0
    }
}

impl fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryKey(***)")
    }
}

#[cfg(test)]
mod tests {
    use zeroize::Zeroizing;

    use super::*;
    use crate::crypto::KEY_LEN;

    fn counting_key() -> RecoveryKey {
        let bytes = std::array::from_fn(|i| u8::try_from(i).unwrap());
        RecoveryKey(Key::from_bytes(Zeroizing::new(bytes)))
    }

    #[test]
    fn parse_reverses_encode() {
        let key = counting_key();
        assert_eq!(
            key.encode().expose(),
            "000G-40R4-0M30-E209-185G-R38E-1W81-24GK-2GAH-C5RR-34D1-P70X-3RFG"
        );
        let parsed = RecoveryKey::parse(key.encode().expose()).unwrap();
        assert_eq!(parsed.as_key().as_bytes(), key.as_key().as_bytes());
    }

    #[test]
    fn text_errors_become_recovery_key_errors() {
        let valid = counting_key().encode();
        let text = valid.expose();
        let cases = [
            ("U", RecoveryKeyError::Malformed),
            ("000G", RecoveryKeyError::Malformed),
            (
                &format!("{}H", &text[..text.len() - 1]),
                RecoveryKeyError::InvalidLastCharacter,
            ),
        ];
        for (text, error) in cases {
            assert_eq!(RecoveryKey::parse(text).err(), Some(error), "{text}");
        }
    }

    #[test]
    fn generated_keys_differ() {
        let first = RecoveryKey::generate().unwrap();
        let second = RecoveryKey::generate().unwrap();
        assert_ne!(first.as_key().as_bytes(), second.as_key().as_bytes());
        assert_ne!(first.as_key().as_bytes(), &[0; KEY_LEN]);
    }

    #[test]
    fn debug_never_shows_the_key() {
        assert_eq!(format!("{:?}", counting_key()), "RecoveryKey(***)");
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                RecoveryKeyError::Malformed,
                "clé de récupération mal saisie : 13 groupes de 4 caractères séparés par des tirets",
            ),
            (
                RecoveryKeyError::InvalidLastCharacter,
                "dernier caractère de la clé de récupération invalide",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
