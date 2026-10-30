//! Heure courante. Le formatage des dates est dans `pswd-core`, partagé avec le serveur.

use std::time::{SystemTime, UNIX_EPOCH};

use pswd_core::Timestamp;
pub use pswd_core::{backup_day, backup_file_name, format_timestamp, server_backup_file_name};

/// Heure courante : `pswd-core` ne lit jamais l'horloge lui-même.
#[must_use]
pub fn now() -> Timestamp {
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
    fn now_gives_the_current_time() {
        // 1ᵉʳ janvier 2026 en millisecondes.
        assert!(now() > Timestamp::from_millis(1_767_225_600_000));
    }
}
