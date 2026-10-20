//! Day buckets in America/Vancouver.
use worker::Date;

// Days are Mackenzie's days: America/Vancouver, not UTC.
// Rows written before this change (deployed 2026-10-19 evening PDT) are UTC days.
pub fn day_before(n: i64) -> String {
    let utc_ms = Date::now().as_millis() as i64;
    let (y, m, d) = civil_from_days((utc_ms + pacific_offset_ms(utc_ms)).div_euclid(86_400_000) - n);
    format!("{y:04}-{m:02}-{d:02}")
}

/// British Columbia's offset from UTC (tzdata 2026d). From 2026-11-01 09:00 UTC BC stays on UTC-7 all year.
/// Before that: -7 h from the second Sunday of March 10:00 UTC to the first Sunday of November 09:00 UTC, else -8 h.
fn pacific_offset_ms(utc_ms: i64) -> i64 {
    const PERMANENT_UTC_MINUS_7: i64 = 1_793_523_600_000; // 2026-11-01T09:00:00Z
    if utc_ms >= PERMANENT_UTC_MINUS_7 {
        return -7 * 3_600_000;
    }
    let (y, _, _) = civil_from_days(utc_ms.div_euclid(86_400_000));
    let nth_sunday = |m: i64, nth: i64| {
        let first = days_from_civil(y, m, 1);
        let to_sunday = (3 - first).rem_euclid(7); // day 0 (1970-01-01) was a Thursday, so day 3 was a Sunday
        first + to_sunday + 7 * (nth - 1)
    };
    let start = nth_sunday(3, 2) * 86_400_000 + 10 * 3_600_000;
    let end = nth_sunday(11, 1) * 86_400_000 + 9 * 3_600_000;
    if (start..end).contains(&utc_ms) { -7 * 3_600_000 } else { -8 * 3_600_000 }
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

