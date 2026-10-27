pub const MAX_VAULT_NAME_LEN: usize = 64;

/// Noms de fichiers interdits par Windows, quelle que soit la casse.
const RESERVED_NAMES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Le nom d'un coffre sert de nom de dossier : lettres ASCII, chiffres, `-` et `_`, ni
/// séparateur, ni `..`, ni nom réservé par Windows.
#[must_use]
pub fn is_valid_vault_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_VAULT_NAME_LEN
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && !RESERVED_NAMES.contains(&name.to_ascii_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_folder_names_are_refused() {
        let too_long = "a".repeat(MAX_VAULT_NAME_LEN + 1);
        for name in [
            "",
            "a/b",
            "a\\b",
            "..",
            "coffre perso",
            "é",
            "CON",
            "lpt1",
            &too_long,
        ] {
            assert!(!is_valid_vault_name(name), "{name:?}");
        }
    }

    #[test]
    fn safe_folder_names_are_accepted() {
        let longest = "a".repeat(MAX_VAULT_NAME_LEN);
        for name in ["Perso-2_x", "console", "Z", &longest] {
            assert!(is_valid_vault_name(name), "{name:?}");
        }
    }
}
