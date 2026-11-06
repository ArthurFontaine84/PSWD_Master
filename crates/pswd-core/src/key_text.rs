//! Écriture lisible des clés de 32 octets recopiées à la main (récupération, connexion) :
//! Base32 Crockford, `XXXX-XXXX-…` (13 groupes de 4 caractères).

use zeroize::Zeroizing;

use crate::Secret;
use crate::crypto::KEY_LEN;

/// Base32 Crockford : sans I, L, O ni U, pour éviter les confusions à la recopie.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const SYMBOLS: usize = (KEY_LEN * 8).div_ceil(5);
pub(crate) const GROUP_LEN: usize = 4;
pub(crate) const GROUPS: usize = SYMBOLS.div_ceil(GROUP_LEN);
const SEPARATOR: char = '-';
/// Longueur de l'écriture complète, tirets compris.
const TEXT_LEN: usize = SYMBOLS + (SYMBOLS - 1) / GROUP_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyTextError {
    /// Pas sous la forme `XXXX-XXXX-…` avec les caractères de l'alphabet.
    Malformed,
    /// Bien formée, mais aucune clé ne s'écrit ainsi.
    InvalidLastCharacter,
}

/// Texte à montrer à l'utilisateur.
pub(crate) fn encode(bytes: &[u8; KEY_LEN]) -> Secret {
    let mut text = String::with_capacity(TEXT_LEN);
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
    for &byte in bytes {
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

/// Lit une clé saisie par l'utilisateur : exactement l'écriture montrée, tirets compris,
/// en majuscules ou en minuscules. Seuls les espaces autour, qu'un copier-coller ajoute
/// souvent, sont ignorés.
pub(crate) fn decode(text: &str) -> Result<Zeroizing<[u8; KEY_LEN]>, KeyTextError> {
    let text = text.trim();
    if text.chars().count() != TEXT_LEN {
        return Err(KeyTextError::Malformed);
    }
    let mut values = Zeroizing::new(Vec::with_capacity(SYMBOLS));
    for (position, c) in text.chars().enumerate() {
        if position % (GROUP_LEN + 1) == GROUP_LEN {
            if c != SEPARATOR {
                return Err(KeyTextError::Malformed);
            }
        } else {
            values.push(symbol_value(c).ok_or(KeyTextError::Malformed)?);
        }
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
        return Err(KeyTextError::InvalidLastCharacter);
    }
    Ok(bytes)
}

fn symbol_value(c: char) -> Option<u8> {
    let c = c.to_ascii_uppercase();
    let position = ALPHABET
        .iter()
        .position(|&symbol| char::from(symbol) == c)?;
    u8::try_from(position).ok()
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    /// Écriture de [`counting`].
    const COUNTING: &str = "000G-40R4-0M30-E209-185G-R38E-1W81-24GK-2GAH-C5RR-34D1-P70X-3RFG";

    fn counting() -> [u8; KEY_LEN] {
        std::array::from_fn(|i| u8::try_from(i).unwrap())
    }

    // Valeurs de référence : module `base64` de Python, alphabet transposé en Crockford.
    #[test]
    fn encoding_matches_reference() {
        assert_eq!(encode(&counting()).expose(), COUNTING);
        assert_eq!(
            encode(&[0xFF; KEY_LEN]).expose(),
            "ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZZ-ZZZG"
        );
    }

    #[test]
    fn decode_accepts_both_cases_and_surrounding_spaces() {
        let upper = COUNTING;
        for text in [
            upper.to_owned(),
            upper.to_lowercase(),
            "000g-40R4-0m30-E209-185g-r38e-1w81-24GK-2gah-C5RR-34d1-P70X-3rfg".to_owned(),
            format!(
                "  {upper}	
"
            ),
        ] {
            assert_eq!(*decode(&text).unwrap(), counting(), "{text}");
        }
    }

    #[test]
    fn decode_requires_the_exact_layout() {
        let upper = COUNTING;
        let cases = [
            "000G40R40M30E209185GR38E1W8124GK2GAHC5RR34D1P70X3RFG".to_owned(),
            upper.replace('-', " "),
            upper.replace('-', "_"),
            upper.replacen('-', "", 1) + "-",
            format!("-{}", &upper[..upper.len() - 1]),
            upper.replacen('E', " ", 1),
            String::new(),
            "000G".to_owned(),
            format!("{upper}-0000"),
        ];
        for text in &cases {
            assert_eq!(decode(text).err(), Some(KeyTextError::Malformed), "{text}");
        }
    }

    #[test]
    fn lookalikes_and_foreign_characters_are_refused() {
        let upper = COUNTING;
        for replacement in ["O", "o", "I", "L", "l", "U", "*", "é"] {
            let text = upper.replacen('0', replacement, 1);
            assert_eq!(decode(&text).err(), Some(KeyTextError::Malformed), "{text}");
        }
    }

    #[test]
    fn padding_bits_must_be_zero() {
        let valid = encode(&counting());
        let text = format!("{}H", &valid.expose()[..valid.expose().len() - 1]);
        assert_eq!(
            decode(&text).err(),
            Some(KeyTextError::InvalidLastCharacter)
        );
    }

    proptest! {
        #[test]
        fn decode_reverses_encode(bytes in prop::array::uniform32(any::<u8>())) {
            prop_assert_eq!(*decode(encode(&bytes).expose()).unwrap(), bytes);
        }
    }
}
