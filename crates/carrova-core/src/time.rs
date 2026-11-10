//! The one clock and calendar of the engine and its front-ends: Unix
//! seconds and UTC calendar dates, without a date-time dependency.

use std::time::{SystemTime, UNIX_EPOCH};

const SECS_PER_DAY: u64 = 86_400;

/// Current Unix time in seconds (`0` before the epoch).
#[must_use]
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `YYYYMMDD-HHMMSS` (UTC) for `secs` since the epoch (the session id
/// format).
#[must_use]
pub fn timestamp(secs: u64) -> String {
    let tod = secs % SECS_PER_DAY;
    format!(
        "{}-{:02}{:02}{:02}",
        date(secs),
        tod / 3600,
        (tod / 60) % 60,
        tod % 60
    )
}

/// `YYYYMMDD` (UTC) for `secs` since the epoch (today:
/// `date(now_unix())`; keep-both names).
#[must_use]
pub fn date(secs: u64) -> String {
    let (y, m, d) = civil_from_secs(secs);
    format!("{y:04}{m:02}{d:02}")
}

/// `YYYY-MM-DD` (UTC) for `secs` since the epoch (dates shown to people).
#[must_use]
pub fn utc_date(secs: u64) -> String {
    let (y, m, d) = civil_from_secs(secs);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The UTC calendar day of `secs` since the epoch.
fn civil_from_secs(secs: u64) -> (i64, u32, u32) {
    civil_from_days(i64::try_from(secs / SECS_PER_DAY).unwrap_or(i64::MAX))
}

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian
/// (H. Hinnant's `civil_from_days`).
#[must_use]
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days.saturating_add(719_468);
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (
        y,
        u32::try_from(m).unwrap_or(1),
        u32::try_from(d).unwrap_or(1),
    )
}
