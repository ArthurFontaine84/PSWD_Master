/// Longueur maximale d'un nom de coffre, en caractères.
pub const MAX_VAULT_NAME_LEN: usize = 64;

/// Taille maximale d'un nom en octets : sa longueur tient sur un octet dans une sauvegarde
/// (§7). Seul un nom de 64 caractères tous codés sur 4 octets (emoji) la dépasse.
const MAX_VAULT_NAME_BYTES: usize = 255;

/// Le nom d'un coffre est un texte libre, espaces et accents compris : il ne sert pas de nom
/// de dossier. Il n'est ni vide, ni entouré d'espaces, n'a aucun caractère de contrôle et
/// compte 64 caractères au plus. Deux noms ne sont égaux que s'ils sont identiques.
#[must_use]
pub fn is_valid_vault_name(name: &str) -> bool {
    !name.is_empty()
        && name.trim() == name
        && name.len() <= MAX_VAULT_NAME_BYTES
        && name.chars().count() <= MAX_VAULT_NAME_LEN
        && !name.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_names_are_refused() {
        let too_long = "a".repeat(MAX_VAULT_NAME_LEN + 1);
        let too_heavy = "🔒".repeat(MAX_VAULT_NAME_LEN);
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
        let heaviest = "🔒".repeat(MAX_VAULT_NAME_LEN - 1);
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
