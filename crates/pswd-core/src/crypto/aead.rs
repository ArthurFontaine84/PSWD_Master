use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use super::{CryptoError, KEY_LEN, Key, fill_random, internal_error};

pub const NONCE_LEN: usize = 24;
pub const TAG_LEN: usize = 16;

/// Résultat d'un chiffrement : le nonce doit être conservé avec le chiffré.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    pub nonce: [u8; NONCE_LEN],
    pub ciphertext: Vec<u8>,
}

/// Chiffre et authentifie `plaintext` ; `aad` est authentifié sans être chiffré.
/// Le nonce est tiré au hasard : avec 192 bits, une collision est exclue en pratique.
///
/// # Errors
///
/// [`CryptoError::RandomUnavailable`] si le générateur aléatoire échoue.
pub fn seal(key: &Key, plaintext: &[u8], aad: &[u8]) -> Result<Sealed, CryptoError> {
    let mut nonce = [0; NONCE_LEN];
    fill_random(&mut nonce)?;
    seal_with_nonce(key, nonce, plaintext, aad)
}

fn seal_with_nonce(
    key: &Key,
    nonce: [u8; NONCE_LEN],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<Sealed, CryptoError> {
    let ciphertext = cipher(key)
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(internal_error)?;
    Ok(Sealed { nonce, ciphertext })
}

/// # Errors
///
/// [`CryptoError::Decryption`] si la clé, le nonce, `aad` ou le chiffré ne correspondent pas.
pub fn open(
    key: &Key,
    nonce: &[u8; NONCE_LEN],
    ciphertext: &[u8],
    aad: &[u8],
) -> Result<Zeroizing<Vec<u8>>, CryptoError> {
    cipher(key)
        .decrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| CryptoError::Decryption)
}

/// Chiffre une clé avec une clé d'enveloppe.
///
/// # Errors
///
/// [`CryptoError::RandomUnavailable`] si le générateur aléatoire échoue.
pub fn wrap_key(wrapping_key: &Key, key: &Key, aad: &[u8]) -> Result<Sealed, CryptoError> {
    seal(wrapping_key, key.as_bytes(), aad)
}

/// # Errors
///
/// [`CryptoError::Decryption`] si l'enveloppe ne s'ouvre pas avec cette clé ou ne contient pas
/// une clé.
pub fn unwrap_key(wrapping_key: &Key, wrapped: &Sealed, aad: &[u8]) -> Result<Key, CryptoError> {
    let opened = open(wrapping_key, &wrapped.nonce, &wrapped.ciphertext, aad)?;
    let bytes: &[u8; KEY_LEN] = opened
        .as_slice()
        .try_into()
        .map_err(|_| CryptoError::Decryption)?;
    let mut key = Zeroizing::new([0; KEY_LEN]);
    key.copy_from_slice(bytes);
    Ok(Key::from_bytes(key))
}

fn cipher(key: &Key) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(key.as_bytes().into())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::crypto::hex;

    fn key(fill: u8) -> Key {
        Key::from_bytes(Zeroizing::new([fill; KEY_LEN]))
    }

    // Valeur de référence calculée avec PyNaCl (libsodium).
    #[test]
    fn matches_reference_xchacha20poly1305() {
        let key = Key::from_bytes(Zeroizing::new(std::array::from_fn(|i| {
            0x80 + u8::try_from(i).unwrap()
        })));
        let nonce = std::array::from_fn(|i| 0x40 + u8::try_from(i).unwrap());
        let sealed = seal_with_nonce(&key, nonce, b"hello vault", b"PSWD").unwrap();
        assert_eq!(
            hex(&sealed.ciphertext),
            "99691f9834d0823b8e7e038935a9af2ae040503b5b99afbaefe1b8"
        );
        assert_eq!(sealed.nonce, nonce);
    }

    #[test]
    fn each_seal_uses_a_new_nonce() {
        let first = seal(&key(1), b"same", b"").unwrap();
        let second = seal(&key(1), b"same", b"").unwrap();
        assert_ne!(first.nonce, second.nonce);
        assert_ne!(first.ciphertext, second.ciphertext);
    }

    #[test]
    fn wrong_key_nonce_or_aad_is_rejected() {
        let sealed = seal(&key(1), b"secret", b"header").unwrap();
        let mut other_nonce = sealed.nonce;
        other_nonce[0] ^= 1;
        assert_eq!(
            open(&key(2), &sealed.nonce, &sealed.ciphertext, b"header"),
            Err(CryptoError::Decryption)
        );
        assert_eq!(
            open(&key(1), &other_nonce, &sealed.ciphertext, b"header"),
            Err(CryptoError::Decryption)
        );
        assert_eq!(
            open(&key(1), &sealed.nonce, &sealed.ciphertext, b"HEADER"),
            Err(CryptoError::Decryption)
        );
    }

    #[test]
    fn wrapped_key_round_trips() {
        let vault_key = key(9);
        let wrapped = wrap_key(&key(1), &vault_key, b"aad").unwrap();
        assert_eq!(wrapped.ciphertext.len(), KEY_LEN + TAG_LEN);
        let unwrapped = unwrap_key(&key(1), &wrapped, b"aad").unwrap();
        assert_eq!(unwrapped.as_bytes(), vault_key.as_bytes());
    }

    #[test]
    fn unwrap_rejects_a_wrong_key_or_aad() {
        let wrapped = wrap_key(&key(1), &key(9), b"aad").unwrap();
        assert_eq!(
            unwrap_key(&key(2), &wrapped, b"aad").err(),
            Some(CryptoError::Decryption)
        );
        assert_eq!(
            unwrap_key(&key(1), &wrapped, b"other").err(),
            Some(CryptoError::Decryption)
        );
    }

    #[test]
    fn unwrap_rejects_content_that_is_not_a_key() {
        let sealed = seal(&key(1), b"too short", b"").unwrap();
        assert_eq!(
            unwrap_key(&key(1), &sealed, b"").err(),
            Some(CryptoError::Decryption)
        );
    }

    proptest! {
        #[test]
        fn open_reverses_seal(
            plaintext in prop::collection::vec(any::<u8>(), 0..256),
            aad in prop::collection::vec(any::<u8>(), 0..64),
        ) {
            let sealed = seal(&key(7), &plaintext, &aad).unwrap();
            let opened = open(&key(7), &sealed.nonce, &sealed.ciphertext, &aad).unwrap();
            prop_assert_eq!(opened.as_slice(), plaintext.as_slice());
        }

        #[test]
        fn any_altered_byte_is_detected(
            plaintext in prop::collection::vec(any::<u8>(), 0..64),
            position in any::<prop::sample::Index>(),
            flip in 1..=255_u8,
        ) {
            let mut sealed = seal(&key(7), &plaintext, b"").unwrap();
            let index = position.index(sealed.ciphertext.len());
            sealed.ciphertext[index] ^= flip;
            prop_assert_eq!(
                open(&key(7), &sealed.nonce, &sealed.ciphertext, b""),
                Err(CryptoError::Decryption)
            );
        }
    }
}
