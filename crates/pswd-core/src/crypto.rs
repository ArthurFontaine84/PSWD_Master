mod aead;
mod kdf;

use std::fmt;

use zeroize::Zeroizing;

pub use aead::{NONCE_LEN, Sealed, open, seal};
pub use kdf::{KdfParams, SALT_LEN, Salt, password_wrapping_key, recovery_wrapping_key};

pub const KEY_LEN: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    RandomUnavailable,
    InvalidKdfParams,
    Internal,
    Decryption,
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RandomUnavailable => "générateur aléatoire du système indisponible",
            Self::InvalidKdfParams => "paramètres de dérivation de clé invalides",
            Self::Internal => "erreur interne de la bibliothèque cryptographique",
            Self::Decryption => "déchiffrement impossible : clé incorrecte ou données altérées",
        })
    }
}

impl std::error::Error for CryptoError {}

/// Clé symétrique de 256 bits, effacée de la mémoire à la libération.
#[derive(Clone)]
pub struct Key(Zeroizing<[u8; KEY_LEN]>);

impl Key {
    /// # Errors
    ///
    /// [`CryptoError::RandomUnavailable`] si le générateur aléatoire du système échoue.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut bytes = Zeroizing::new([0; KEY_LEN]);
        fill_random(bytes.as_mut_slice())?;
        Ok(Self(bytes))
    }

    pub(crate) fn from_bytes(bytes: Zeroizing<[u8; KEY_LEN]>) -> Self {
        Self(bytes)
    }

    pub(crate) fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(***)")
    }
}

/// Erreur d'une bibliothèque crypto qui ne peut pas survenir avec nos entrées : paramètres déjà
/// validés, tailles fixes, messages très loin des limites des algorithmes.
// Exclue de la couverture : aucun test ne peut l'atteindre.
#[cfg_attr(coverage_nightly, coverage(off))]
fn internal_error<E>(_: E) -> CryptoError {
    CryptoError::Internal
}

fn fill_random(buffer: &mut [u8]) -> Result<(), CryptoError> {
    getrandom::fill(buffer).map_err(|_| CryptoError::RandomUnavailable)
}

#[cfg(test)]
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, byte| {
        write!(out, "{byte:02x}").unwrap();
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_keys_differ() {
        let first = Key::generate().unwrap();
        let second = Key::generate().unwrap();
        assert_ne!(first.as_bytes(), second.as_bytes());
        assert_ne!(first.as_bytes(), &[0; KEY_LEN]);
    }

    #[test]
    fn debug_never_shows_the_key() {
        let key = Key::from_bytes(Zeroizing::new([0xAB; KEY_LEN]));
        assert_eq!(format!("{key:?}"), "Key(***)");
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                CryptoError::RandomUnavailable,
                "générateur aléatoire du système indisponible",
            ),
            (
                CryptoError::InvalidKdfParams,
                "paramètres de dérivation de clé invalides",
            ),
            (
                CryptoError::Internal,
                "erreur interne de la bibliothèque cryptographique",
            ),
            (
                CryptoError::Decryption,
                "déchiffrement impossible : clé incorrecte ou données altérées",
            ),
        ];
        for (error, message) in messages {
            assert_eq!(error.to_string(), message);
        }
    }
}
