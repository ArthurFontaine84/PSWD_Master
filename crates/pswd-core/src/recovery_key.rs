use std::fmt;

use zeroize::Zeroizing;

use crate::Secret;
use crate::crypto::{CryptoError, KEY_LEN, Key};

/// Base32 Crockford : sans I, L, O ni U, pour éviter les confusions à la recopie.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const SYMBOLS: usize = (KEY_LEN * 8).div_ceil(5);
const GROUP_LEN: usize = 4;
const SEPARATOR: char = '-';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryKeyError {
    InvalidCharacter,
    WrongLength,
    InvalidLastCharacter,
}

impl fmt::Display for RecoveryKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCharacter => f.write_str("caractère invalide dans la clé de récupération"),
            Self::WrongLength => write!(
                f,
                "la clé de récupération doit compter {SYMBOLS} caractères"
            ),
            Self::InvalidLastCharacter => {
                f.write_str("dernier caractère de la clé de récupération invalide")
            }
        }
    }
}

impl std::error::Error for RecoveryKeyError {}

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
        let mut text = String::with_capacity(SYMBOLS + (SYMBOLS - 1) / GROUP_LEN);
        let mut buffer: u16 = 0;
        let mut bits = 0;
        let mut written = 0;
        let mut push = |value: u16| {
            if written > 0 && written % GROUP_LEN == 0 {
                text.push(SEPARATOR);
            }
            text.push(char::from(ALPHABET[usize::from(value & 0x1F)]));
            written += 1;
        };
        for &byte in self.0.as_bytes() {
            buffer = (buffer << 8) + u16::from(byte);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                push(buffer >> bits);
            }
            buffer &= (1 << bits) - 1;
        }
        // Les bits restants sont complétés par des zéros.
        push(buffer << (5 - bits));
        Secret::new(text)
    }

    /// Lit une clé saisie par l'utilisateur : tirets et espaces ignorés, majuscules ou
    /// minuscules, `O` lu `0`, `I` et `L` lus `1`.
    ///
    /// # Errors
    ///
    /// [`RecoveryKeyError`] si le texte ne peut pas être une clé de récupération.
    pub fn parse(text: &str) -> Result<Self, RecoveryKeyError> {
        let mut values = Zeroizing::new(Vec::with_capacity(text.len()));
        for c in text.chars() {
            if c != SEPARATOR && !c.is_whitespace() {
                values.push(symbol_value(c).ok_or(RecoveryKeyError::InvalidCharacter)?);
            }
        }
        if values.len() != SYMBOLS {
            return Err(RecoveryKeyError::WrongLength);
        }
        let mut bytes = Zeroizing::new([0; KEY_LEN]);
        let mut buffer: u16 = 0;
        let mut bits = 0;
        let mut index = 0;
        for &value in values.iter() {
            buffer = (buffer << 5) + u16::from(value);
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                // Moins de 256 : `buffer` ne garde que `bits + 8` bits significatifs.
                #[allow(clippy::cast_possible_truncation)]
                let byte = (buffer >> bits) as u8;
                bytes[index] = byte;
                index += 1;
                buffer &= (1 << bits) - 1;
            }
        }
        // Les bits de complément doivent être nuls : chaque clé n'a qu'une seule écriture.
        if buffer != 0 {
            return Err(RecoveryKeyError::InvalidLastCharacter);
        }
        Ok(Self(Key::from_bytes(bytes)))
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

fn symbol_value(c: char) -> Option<u8> {
    let c = match c.to_ascii_uppercase() {
        'O' => '0',
        'I' | 'L' => '1',
        other => other,
    };
    let position = ALPHABET
        .iter()
        .position(|&symbol| char::from(symbol) == c)?;
    u8::try_from(position).ok()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn key_from(bytes: [u8; KEY_LEN]) -> RecoveryKey {
        RecoveryKey(Key::from_bytes(Zeroizing::new(bytes)))
    }

    fn counting_key() -> RecoveryKey {
        key_from(std::array::from_fn(|i| u8::try_from(i).unwrap()))
    }

    // Valeurs de référence : module `base64` de Python, alphabet transposé en Crockford.
    #[test]
    fn encoding_matches_reference() {
        assert_eq!(
            counting_key().encode().expose(),
            "000G-40R4-0M30-E209-185G-R38E-1W81-24GK-2GAH-C5RR-34D1-P70X-3RFG"
        );
        assert_eq!(
            key_from([0xFF; KEY_LEN]).encode().expose(),
            "ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZG"
        );
    }

    #[test]
    fn parse_tolerates_case_spaces_and_lookalikes() {
        let text = "ooog 40r4 om3o e2o9 l85g r38e iw8l 24gk 2gah c5rr 34d1 p7ox 3rfg";
        let parsed = RecoveryKey::parse(text).unwrap();
        assert_eq!(
            parsed.as_key().as_bytes(),
            counting_key().as_key().as_bytes()
        );
    }

    #[test]
    fn parse_accepts_unseparated_text() {
        let text = "000G40R40M30E209185GR38E1W8124GK2GAHC5RR34D1P70X3RFG";
        let parsed = RecoveryKey::parse(text).unwrap();
        assert_eq!(
            parsed.as_key().as_bytes(),
            counting_key().as_key().as_bytes()
        );
    }

    #[test]
    fn invalid_characters_are_rejected() {
        for text in ["U", "*", "é", "000G_40R4"] {
            assert_eq!(
                RecoveryKey::parse(text).err(),
                Some(RecoveryKeyError::InvalidCharacter),
                "{text}"
            );
        }
    }

    #[test]
    fn wrong_length_is_rejected() {
        let valid = counting_key().encode();
        let text = valid.expose();
        for wrong in [&text[..text.len() - 1], &format!("{text}0"), ""] {
            assert_eq!(
                RecoveryKey::parse(wrong).err(),
                Some(RecoveryKeyError::WrongLength),
                "{wrong}"
            );
        }
    }

    #[test]
    fn padding_bits_must_be_zero() {
        let valid = counting_key().encode();
        let text = format!("{}H", &valid.expose()[..valid.expose().len() - 1]);
        assert_eq!(
            RecoveryKey::parse(&text).err(),
            Some(RecoveryKeyError::InvalidLastCharacter)
        );
    }

    #[test]
    fn generated_keys_differ() {
        let first = RecoveryKey::generate().unwrap();
        let second = RecoveryKey::generate().unwrap();
        assert_ne!(first.as_key().as_bytes(), second.as_key().as_bytes());
    }

    #[test]
    fn debug_never_shows_the_key() {
        assert_eq!(format!("{:?}", counting_key()), "RecoveryKey(***)");
    }

    #[test]
    fn errors_have_explicit_messages() {
        let messages = [
            (
                RecoveryKeyError::InvalidCharacter,
                "caractère invalide dans la clé de récupération",
            ),
            (
                RecoveryKeyError::WrongLength,
                "la clé de récupération doit compter 52 caractères",
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

    proptest! {
        #[test]
        fn parse_reverses_encode(bytes in prop::array::uniform32(any::<u8>())) {
            let key = key_from(bytes);
            let parsed = RecoveryKey::parse(key.encode().expose()).unwrap();
            prop_assert_eq!(parsed.as_key().as_bytes(), &bytes);
        }
    }
}
