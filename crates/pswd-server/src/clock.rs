use std::time::{SystemTime, UNIX_EPOCH};

use pswd_core::Timestamp;

/// Heure courante : `pswd-core` ne lit jamais l'horloge lui-même.
pub(crate) fn now() -> Timestamp {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        });
    Timestamp::from_millis(millis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_after_2025() {
        assert!(now() > Timestamp::from_millis(1_735_689_600_000));
    }
}
