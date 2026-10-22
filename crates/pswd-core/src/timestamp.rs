use serde::{Deserialize, Serialize};

/// Instant UTC en millisecondes depuis l'époque Unix.
///
/// `pswd-core` ne lit jamais l'horloge : l'appelant fournit l'heure courante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(i64);

impl Timestamp {
    #[must_use]
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    #[must_use]
    pub const fn as_millis(self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn millis_round_trip() {
        assert_eq!(
            Timestamp::from_millis(1_700_000_000_123).as_millis(),
            1_700_000_000_123
        );
    }

    #[test]
    fn later_instant_is_greater() {
        assert!(Timestamp::from_millis(2) > Timestamp::from_millis(1));
    }
}
