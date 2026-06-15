/// Calibration offset for the supply-day rotation formula. Adjust and redeploy
/// if the highlighted item doesn't match what's shown in-game.
pub const ROTATION_EPOCH_DAYS: i64 = 0;

/// Computes the supply day from a unix timestamp.
/// Formula: (unix_timestamp_seconds - 72000) / 86400 + ROTATION_EPOCH_DAYS
pub fn supply_day(now_unix: i64) -> i64 {
    (now_unix - 72_000) / 86_400 + ROTATION_EPOCH_DAYS
}

/// Returns the index (0-based) of today's pool item for a given pool size.
/// Returns None if pool_size == 0.
pub fn today_index(now_unix: i64, pool_size: usize) -> Option<usize> {
    if pool_size == 0 {
        return None;
    }
    let day = supply_day(now_unix);
    Some(day.rem_euclid(pool_size as i64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supply_day() {
        // Test a known unix timestamp
        let test_unix = 1_640_000_000i64; // arbitrary value
        let day = supply_day(test_unix);
        assert!(day > 0);
    }

    #[test]
    fn test_today_index() {
        let test_unix = 1_640_000_000i64;
        let index = today_index(test_unix, 3);
        assert!(index.is_some());
        assert!(index.unwrap() < 3);

        let empty = today_index(test_unix, 0);
        assert!(empty.is_none());
    }
}
