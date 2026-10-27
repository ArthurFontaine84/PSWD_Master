use std::fmt;
use std::ops::RangeInclusive;

use rand::{CryptoRng, RngExt};

use crate::Secret;

const UPPERCASE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWERCASE: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const DIGITS: &[u8] = b"0123456789";
/// Sans espace, guillemets, antislash ni accent grave : ils posent problème dans un terminal ou
/// un formulaire.
const SYMBOLS: &[u8] = b"!#$%&()*+,-./:;<=>?@[]^_{|}~";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorError {
    InvalidLength,
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength => write!(
                f,
                "la longueur doit être comprise entre {} et {}",
                PasswordOptions::LENGTH.start(),
                PasswordOptions::LENGTH.end()
            ),
        }
    }
}

impl std::error::Error for GeneratorError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordOptions {
    pub length: usize,
    pub symbols: bool,
}

impl PasswordOptions {
    const LENGTH: RangeInclusive<usize> = 8..=24;
}

impl Default for PasswordOptions {
    fn default() -> Self {
        Self {
            length: 24,
            symbols: true,
        }
    }
}

/// Mot de passe aléatoire contenant au moins un caractère de chaque classe active.
///
/// Les candidats incomplets sont rejetés puis retirés : tous les mots de passe valides restent
/// équiprobables. `CryptoRng` impose un générateur adapté à la cryptographie.
///
/// # Errors
///
/// [`GeneratorError::InvalidLength`] si la longueur sort des bornes autorisées.
pub fn generate_password<R: CryptoRng>(
    options: PasswordOptions,
    rng: &mut R,
) -> Result<Secret, GeneratorError> {
    if !PasswordOptions::LENGTH.contains(&options.length) {
        return Err(GeneratorError::InvalidLength);
    }
    let classes: &[&[u8]] = if options.symbols {
        &[UPPERCASE, LOWERCASE, DIGITS, SYMBOLS]
    } else {
        &[UPPERCASE, LOWERCASE, DIGITS]
    };
    let alphabet = classes.concat();
    // Même tampon à chaque essai : il n'est jamais réalloué, chaque candidat écrase le précédent.
    let mut password = String::with_capacity(options.length);
    loop {
        password.clear();
        for _ in 0..options.length {
            password.push(char::from(alphabet[rng.random_range(0..alphabet.len())]));
        }
        let complete = classes
            .iter()
            .all(|class| password.bytes().any(|byte| class.contains(&byte)));
        if complete {
            return Ok(Secret::new(password));
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    use super::*;

    fn generate(length: usize, symbols: bool, seed: u64) -> Result<Secret, GeneratorError> {
        let options = PasswordOptions { length, symbols };
        generate_password(options, &mut StdRng::seed_from_u64(seed))
    }

    fn only_known_characters(password: &str) -> bool {
        let known = [UPPERCASE, LOWERCASE, DIGITS, SYMBOLS].concat();
        password.bytes().all(|byte| known.contains(&byte))
    }

    fn contains_any(password: &str, class: &[u8]) -> bool {
        password.bytes().any(|byte| class.contains(&byte))
    }

    #[test]
    fn default_is_24_characters_with_symbols() {
        let options = PasswordOptions::default();
        assert_eq!(options.length, 24);
        assert!(options.symbols);
        let password = generate_password(options, &mut StdRng::seed_from_u64(1)).unwrap();
        assert_eq!(password.expose().len(), 24);
        assert!(contains_any(password.expose(), SYMBOLS));
    }

    #[test]
    fn length_bounds_are_enforced() {
        for length in [8, 24] {
            assert_eq!(generate(length, true, 1).unwrap().expose().len(), length);
        }
        for length in [0, 7, 25] {
            assert_eq!(
                generate(length, true, 1).err(),
                Some(GeneratorError::InvalidLength),
                "{length}"
            );
        }
    }

    #[test]
    fn every_character_can_be_drawn() {
        let mut rng = StdRng::seed_from_u64(7);
        let mut drawn = Vec::new();
        for _ in 0..100 {
            let password = generate_password(PasswordOptions::default(), &mut rng).unwrap();
            drawn.extend(password.expose().bytes());
        }
        for class in [UPPERCASE, LOWERCASE, DIGITS, SYMBOLS] {
            for byte in class {
                assert!(drawn.contains(byte), "{}", char::from(*byte));
            }
        }
    }

    #[test]
    fn same_seed_gives_same_password_and_seeds_differ() {
        assert_eq!(generate(24, true, 3), generate(24, true, 3));
        assert_ne!(generate(24, true, 3), generate(24, true, 4));
    }

    #[test]
    fn error_message_gives_the_bounds() {
        assert_eq!(
            GeneratorError::InvalidLength.to_string(),
            "la longueur doit être comprise entre 8 et 24"
        );
    }

    proptest! {
        #[test]
        fn password_respects_the_options(
            length in 8..=24_usize,
            symbols in any::<bool>(),
            seed in any::<u64>(),
        ) {
            let password = generate(length, symbols, seed).unwrap();
            let password = password.expose();
            prop_assert_eq!(password.len(), length);
            prop_assert!(contains_any(password, UPPERCASE));
            prop_assert!(contains_any(password, LOWERCASE));
            prop_assert!(contains_any(password, DIGITS));
            prop_assert_eq!(contains_any(password, SYMBOLS), symbols);
            prop_assert!(only_known_characters(password));
        }
    }
}
