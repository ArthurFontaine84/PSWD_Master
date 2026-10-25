//! Règle d'un nouveau mot de passe maître (SPEC §9). Un mot de passe déjà en place n'est jamais
//! revérifié : il ouvre toujours son coffre.

use pswd_core::Secret;

use crate::AppError;

/// Nombre minimal de caractères (et non d'octets).
pub const MIN_PASSWORD_LENGTH: usize = 12;
/// Nombre minimal de types de caractères parmi minuscules, majuscules, chiffres, symboles.
pub const MIN_CHARACTER_KINDS: usize = 3;

/// Minuscule, majuscule, chiffre, puis tout le reste (symbole, espace, lettre sans casse).
const KINDS: [fn(char) -> bool; 4] = [
    char::is_lowercase,
    char::is_uppercase,
    |c| c.is_ascii_digit(),
    |c| !(c.is_lowercase() || c.is_uppercase() || c.is_ascii_digit()),
];

/// Vérifie qu'un nouveau mot de passe maître est assez fort.
///
/// # Errors
///
/// [`AppError::PasswordTooShort`] sous [`MIN_PASSWORD_LENGTH`] caractères,
/// [`AppError::PasswordTooSimple`] sous [`MIN_CHARACTER_KINDS`] types de caractères.
pub fn check_master_password(password: &Secret) -> Result<(), AppError> {
    let password = password.expose();
    if password.chars().count() < MIN_PASSWORD_LENGTH {
        return Err(AppError::PasswordTooShort);
    }
    let kinds = KINDS
        .iter()
        .filter(|is_kind| password.chars().any(is_kind))
        .count();
    if kinds < MIN_CHARACTER_KINDS {
        return Err(AppError::PasswordTooSimple);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(password: &str) -> Result<(), AppError> {
        check_master_password(&Secret::new(password.to_owned()))
    }

    #[test]
    fn twelve_characters_of_three_kinds_are_enough() {
        for password in [
            "abcdefghij1A",
            "abcdefghij1!",
            "abcdefghijA!",
            "ABCDEFGHIJ1!",
            "Abcdefghij1!",
            "une phrase de passe Longue",
        ] {
            assert_eq!(check(password), Ok(()), "{password}");
        }
    }

    #[test]
    fn short_passwords_are_refused() {
        for password in ["", "Ab1!", "Abcdefghi1!"] {
            assert_eq!(
                check(password),
                Err(AppError::PasswordTooShort),
                "{password}"
            );
        }
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        assert_eq!(check("éééééééé1A!"), Err(AppError::PasswordTooShort));
        assert_eq!(check("ééééééééé1A!"), Ok(()));
    }

    #[test]
    fn passwords_with_fewer_than_three_kinds_are_refused() {
        for password in [
            "abcdefghijkl",
            "ABCDEFGHIJKL",
            "123456789012",
            "!!!!!!!!!!!!",
            "abcdefghijk1",
            "abcdefghijkL",
            "abcdefghijk!",
            "ABCDEFGHIJK1",
            "ABCDEFGHIJK!",
            "12345678901!",
        ] {
            assert_eq!(
                check(password),
                Err(AppError::PasswordTooSimple),
                "{password}"
            );
        }
    }

    #[test]
    fn accented_letters_spaces_and_caseless_letters_are_classified() {
        assert_eq!(check("éèàùâêîôûç1É"), Ok(()));
        assert_eq!(check("éèàùâêîôûçÉÈ"), Err(AppError::PasswordTooSimple));
        assert_eq!(check("abcdefghij 1"), Ok(()));
        assert_eq!(check("密码密码密码密码密码a1"), Ok(()));
    }
}
