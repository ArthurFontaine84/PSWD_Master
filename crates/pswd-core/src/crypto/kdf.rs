use std::ops::RangeInclusive;

use argon2::{Algorithm, Argon2, Params, Version};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use super::{CryptoError, KEY_LEN, Key, fill_random, internal_error};
use crate::Secret;

pub const SALT_LEN: usize = 16;

const RECOVERY_INFO: &[u8] = b"pswd-recovery-v1";

/// Paramètres Argon2id. Ils sont lus depuis le fichier du coffre : les bornes empêchent un
/// fichier malveillant d'imposer un calcul démesuré.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    memory_kib: u32,
    iterations: u32,
    parallelism: u32,
}

impl KdfParams {
    pub const DEFAULT: Self = Self {
        memory_kib: 64 * 1024,
        iterations: 3,
        parallelism: 1,
    };

    const MEMORY_KIB: RangeInclusive<u32> = 8 * 1024..=1024 * 1024;
    const ITERATIONS: RangeInclusive<u32> = 1..=10;
    const PARALLELISM: RangeInclusive<u32> = 1..=8;

    /// # Errors
    ///
    /// [`CryptoError::InvalidKdfParams`] si un paramètre sort des bornes autorisées.
    pub fn new(memory_kib: u32, iterations: u32, parallelism: u32) -> Result<Self, CryptoError> {
        if Self::MEMORY_KIB.contains(&memory_kib)
            && Self::ITERATIONS.contains(&iterations)
            && Self::PARALLELISM.contains(&parallelism)
        {
            Ok(Self {
                memory_kib,
                iterations,
                parallelism,
            })
        } else {
            Err(CryptoError::InvalidKdfParams)
        }
    }

    #[must_use]
    pub fn memory_kib(&self) -> u32 {
        self.memory_kib
    }

    #[must_use]
    pub fn iterations(&self) -> u32 {
        self.iterations
    }

    #[must_use]
    pub fn parallelism(&self) -> u32 {
        self.parallelism
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Salt([u8; SALT_LEN]);

impl Salt {
    /// # Errors
    ///
    /// [`CryptoError::RandomUnavailable`] si le générateur aléatoire du système échoue.
    pub fn generate() -> Result<Self, CryptoError> {
        let mut bytes = [0; SALT_LEN];
        fill_random(&mut bytes)?;
        Ok(Self(bytes))
    }

    #[must_use]
    pub fn from_bytes(bytes: [u8; SALT_LEN]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8; SALT_LEN] {
        &self.0
    }
}

/// Clé d'enveloppe dérivée du mot de passe maître (Argon2id).
///
/// # Errors
///
/// [`CryptoError::Internal`] seulement si Argon2 refuse des paramètres déjà validés.
pub fn password_wrapping_key(
    password: &Secret,
    salt: &Salt,
    params: &KdfParams,
) -> Result<Key, CryptoError> {
    // EXTENSION(keyfile): le contenu du fichier-clé sera fourni à Argon2 comme `secret`.
    let argon2_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(KEY_LEN),
    )
    .map_err(internal_error)?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params);
    let mut key = Zeroizing::new([0; KEY_LEN]);
    argon2
        .hash_password_into(
            password.expose().as_bytes(),
            salt.as_bytes(),
            key.as_mut_slice(),
        )
        .map_err(internal_error)?;
    Ok(Key::from_bytes(key))
}

/// Clé d'enveloppe dérivée de la clé de récupération (HKDF-SHA256).
///
/// # Errors
///
/// [`CryptoError::Internal`] en théorie seulement : 32 octets restent sous la limite HKDF.
pub fn recovery_wrapping_key(recovery_key: &Key) -> Result<Key, CryptoError> {
    let mut key = Zeroizing::new([0; KEY_LEN]);
    Hkdf::<Sha256>::new(None, recovery_key.as_bytes())
        .expand(RECOVERY_INFO, key.as_mut_slice())
        .map_err(internal_error)?;
    Ok(Key::from_bytes(key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::hex;

    fn password(value: &str) -> Secret {
        Secret::new(value.to_owned())
    }

    fn test_salt() -> Salt {
        Salt::from_bytes(std::array::from_fn(|i| u8::try_from(i).unwrap()))
    }

    fn fast_params() -> KdfParams {
        KdfParams::new(8 * 1024, 1, 1).unwrap()
    }

    #[test]
    fn default_params_follow_the_spec() {
        let params = KdfParams::DEFAULT;
        assert_eq!(params.memory_kib(), 64 * 1024);
        assert_eq!(params.iterations(), 3);
        assert_eq!(params.parallelism(), 1);
        assert_eq!(KdfParams::new(64 * 1024, 3, 1), Ok(params));
    }

    #[test]
    fn params_at_the_bounds_are_accepted() {
        let low = KdfParams::new(8 * 1024, 1, 1).unwrap();
        assert_eq!(
            (low.memory_kib(), low.iterations(), low.parallelism()),
            (8 * 1024, 1, 1)
        );
        let high = KdfParams::new(1024 * 1024, 10, 8).unwrap();
        assert_eq!(
            (high.memory_kib(), high.iterations(), high.parallelism()),
            (1024 * 1024, 10, 8)
        );
    }

    #[test]
    fn params_out_of_bounds_are_rejected() {
        let invalid = [
            (8 * 1024 - 1, 3, 1),
            (1024 * 1024 + 1, 3, 1),
            (64 * 1024, 0, 1),
            (64 * 1024, 11, 1),
            (64 * 1024, 3, 0),
            (64 * 1024, 3, 9),
        ];
        for (memory, iterations, parallelism) in invalid {
            assert_eq!(
                KdfParams::new(memory, iterations, parallelism),
                Err(CryptoError::InvalidKdfParams),
                "{memory} {iterations} {parallelism}"
            );
        }
    }

    #[test]
    fn generated_salts_differ() {
        assert_ne!(Salt::generate().unwrap(), Salt::generate().unwrap());
    }

    #[test]
    fn salt_bytes_round_trip() {
        assert_eq!(Salt::from_bytes([7; SALT_LEN]).as_bytes(), &[7; SALT_LEN]);
    }

    // Valeur de référence calculée avec argon2-cffi (implémentation de référence en C).
    #[test]
    fn password_key_matches_reference_argon2id() {
        let key = password_wrapping_key(
            &password("correct horse battery staple"),
            &test_salt(),
            &fast_params(),
        )
        .unwrap();
        assert_eq!(
            hex(key.as_bytes()),
            "9aeef75313e585f492c33c5e12d82e9ad253b7dd78632d537e899cd4eddd789c"
        );
    }

    #[test]
    fn password_key_depends_on_every_input() {
        let reference =
            password_wrapping_key(&password("pass"), &test_salt(), &fast_params()).unwrap();
        let other_password =
            password_wrapping_key(&password("Pass"), &test_salt(), &fast_params()).unwrap();
        let other_salt = password_wrapping_key(
            &password("pass"),
            &Salt::from_bytes([0; SALT_LEN]),
            &fast_params(),
        )
        .unwrap();
        let other_params = password_wrapping_key(
            &password("pass"),
            &test_salt(),
            &KdfParams::new(8 * 1024, 2, 1).unwrap(),
        )
        .unwrap();
        for other in [other_password, other_salt, other_params] {
            assert_ne!(other.as_bytes(), reference.as_bytes());
        }
    }

    // Valeur de référence calculée avec hmac/hashlib (Python), selon la RFC 5869.
    #[test]
    fn recovery_key_matches_reference_hkdf() {
        let recovery = Key::from_bytes(Zeroizing::new(std::array::from_fn(|i| {
            u8::try_from(i).unwrap()
        })));
        let key = recovery_wrapping_key(&recovery).unwrap();
        assert_eq!(
            hex(key.as_bytes()),
            "01311e21852c05a0d6fe75c54836b040543589a55ddeeaa8d22494f230d2ccf9"
        );
    }
}
