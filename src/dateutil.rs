//! Howard Hinnant's civil-calendar algorithm, both directions (issue #115
//! dedupe: `gather::days_from_civil` and `inbox_write::civil_from_days`
//! were separate reimplementations of the same family — one place now).
//! Standard, well-tested algorithm; reimplemented here rather than pulling
//! in a time crate for two conversions.

/// Days since the Unix epoch (1970-01-01) for a proleptic-Gregorian civil
/// date. Used by `gather::parse_iso8601_utc`.
pub(crate) fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (i64::from(m) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Inverse of [`days_from_civil`]: the proleptic-Gregorian civil date for
/// a day count since the Unix epoch. Used by
/// `inbox_write::format_iso8601_utc`.
pub(crate) fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_from_civil_matches_a_known_epoch_offset() {
        // 2026-07-17T00:00:00Z, cross-checked against
        // gather.rs's former direct test via parse_iso8601_utc.
        assert_eq!(days_from_civil(2026, 7, 17), 20_651);
    }

    #[test]
    fn civil_from_days_matches_a_known_epoch_offset() {
        assert_eq!(civil_from_days(20_651), (2026, 7, 17));
    }

    #[test]
    fn round_trips_across_a_range_of_dates() {
        for (y, m, d) in [
            (1970, 1, 1),
            (1969, 12, 31),
            (1900, 3, 1),
            (2000, 2, 29),
            (2026, 7, 17),
            (2400, 1, 1),
        ] {
            let days = days_from_civil(y, m, d);
            assert_eq!(civil_from_days(days), (y, m, d));
        }
    }
}
