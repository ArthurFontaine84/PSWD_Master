use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Self::new)
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

    #[test]
    fn serializes_as_a_plain_string() {
        let secret = Secret::new("hunter2".to_owned());
        let json = serde_json::to_string(&secret).unwrap();
        assert_eq!(json, r#""hunter2""#);
        assert_eq!(serde_json::from_str::<Secret>(&json).unwrap(), secret);
    }
}
