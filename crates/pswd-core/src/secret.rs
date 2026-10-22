use std::fmt;

use zeroize::Zeroizing;

/// Valeur sensible : effacée de la mémoire à la libération, masquée dans `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(Zeroizing::new(value))
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expose_returns_the_original_value() {
        let secret = Secret::new("hunter2".to_owned());
        assert_eq!(secret.expose(), "hunter2");
    }

    #[test]
    fn debug_never_shows_the_value() {
        let secret = Secret::new("hunter2".to_owned());
        assert_eq!(format!("{secret:?}"), "Secret(***)");
    }
}
