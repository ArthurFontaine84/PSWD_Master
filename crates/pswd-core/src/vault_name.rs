/// Longueur maximale d'un nom de coffre, en caractères.
pub const MAX_VAULT_NAME_LEN: usize = 32;

// La longueur d'un nom tient sur un octet dans une sauvegarde (§7) : même codés sur 4 octets,
// ses caractères n'en font pas plus de 255.
const _: () = assert!(MAX_VAULT_NAME_LEN * 4 <= 255);

/// Le nom d'un coffre est un texte libre, espaces et accents compris : il ne sert pas de nom
/// de dossier. Il n'est ni vide, ni entouré d'espaces, n'a aucun caractère de contrôle et
/// compte 32 caractères au plus. Deux noms ne sont égaux que s'ils sont identiques.
#[must_use]
pub fn is_valid_vault_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name.chars().count() <= MAX_VAULT_NAME_LEN
        && !name.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_names_are_refused() {
        let too_long = "a".repeat(MAX_VAULT_NAME_LEN + 1);
        let too_heavy = "🔒".repeat(MAX_VAULT_NAME_LEN + 1);
        for name in [
            "",
            " ",
            " perso",
            "perso ",
            "per\nso",
            "per\tso",
            "per\u{7f}so",
            &too_long,
            &too_heavy,
        ] {
            assert!(!is_valid_vault_name(name), "{name:?}");
        }
    }

    #[test]
    fn free_names_are_accepted() {
        let longest = "é".repeat(MAX_VAULT_NAME_LEN);
        let heaviest = "🔒".repeat(MAX_VAULT_NAME_LEN);
        for name in [
            "Mon coffre",
            "Été 2026",
            "a/b",
            "..",
            "CON",
            "Z",
            &longest,
            &heaviest,
        ] {
            assert!(is_valid_vault_name(name), "{name:?}");
        }
    }
}
