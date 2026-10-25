//! Dates affichées et noms des sauvegardes, en UTC : aucun fuseau horaire à connaître.

use std::time::{SystemTime, UNIX_EPOCH};

use pswd_core::Timestamp;

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

/// Date et heure UTC, à la minute : `2026-10-22 14:05 UTC`.
#[must_use]
pub fn format_timestamp(timestamp: Timestamp) -> String {
    let time = UtcTime::new(timestamp);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02} UTC",
        time.year, time.month, time.day, time.hour, time.minute
    )
}

/// Nom proposé pour un fichier de sauvegarde : `pswd-sauvegarde_2026-10-22_14-05-09.pswd`.
#[must_use]
pub fn backup_file_name(timestamp: Timestamp) -> String {
    format!("pswd-sauvegarde_{}.pswd", backup_stamp(timestamp))
}

/// Date et heure d'une sauvegarde, à la seconde : `2026-10-22_14-05-09`.
fn backup_stamp(timestamp: Timestamp) -> String {
    let time = UtcTime::new(timestamp);
    format!(
        "{:04}-{:02}-{:02}_{:02}-{:02}-{:02}",
        time.year, time.month, time.day, time.hour, time.minute, time.second
    )
}

/// Jour d'une sauvegarde : `2026-10-22`.
#[must_use]
pub fn backup_day(timestamp: Timestamp) -> String {
    let time = UtcTime::new(timestamp);
    format!("{:04}-{:02}-{:02}", time.year, time.month, time.day)
}

struct UtcTime {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
}

impl UtcTime {
    fn new(timestamp: Timestamp) -> Self {
        let seconds = timestamp.as_millis().div_euclid(1_000);
        let (year, month, day) = civil_from_days(seconds.div_euclid(86_400));
        let seconds_of_day = seconds.rem_euclid(86_400);
        Self {
            year,
            month,
            day,
            hour: seconds_of_day / 3_600,
            minute: seconds_of_day % 3_600 / 60,
            second: seconds_of_day % 60,
        }
    }
}

/// Jour du calendrier grégorien à partir du nombre de jours depuis le 1ᵉʳ janvier 1970
/// (algorithme de Howard Hinnant).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_gives_the_current_time() {
        // 1ᵉʳ janvier 2026 en millisecondes.
        assert!(now() > Timestamp::from_millis(1_767_225_600_000));
    }

    #[test]
    fn timestamps_are_formatted_in_utc() {
        let cases = [
            (0, "1970-01-01 00:00 UTC"),
            (951_782_400_000, "2000-02-29 00:00 UTC"),
            (1_735_689_599_999, "2024-12-31 23:59 UTC"),
            (1_767_225_600_000, "2026-01-01 00:00 UTC"),
            (4_102_444_800_000, "2100-01-01 00:00 UTC"),
            (-1, "1969-12-31 23:59 UTC"),
        ];
        for (millis, expected) in cases {
            assert_eq!(format_timestamp(Timestamp::from_millis(millis)), expected);
        }
    }

    #[test]
    fn backup_file_names_are_precise_to_the_second() {
        let cases = [
            (0, "pswd-sauvegarde_1970-01-01_00-00-00.pswd"),
            (
                1_761_229_809_999,
                "pswd-sauvegarde_2025-10-23_14-30-09.pswd",
            ),
            (-1, "pswd-sauvegarde_1969-12-31_23-59-59.pswd"),
        ];
        for (millis, expected) in cases {
            assert_eq!(backup_file_name(Timestamp::from_millis(millis)), expected);
        }
    }

    #[test]
    fn backup_day_is_the_date_alone() {
        assert_eq!(
            backup_day(Timestamp::from_millis(1_761_229_809_999)),
            "2025-10-23"
        );
        assert_eq!(backup_day(Timestamp::from_millis(-1)), "1969-12-31");
    }
}
