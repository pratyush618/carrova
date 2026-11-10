//! The wall clock reports are stamped with.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Time since the Unix epoch (zero if the clock is before 1970: a report
/// is still worth keeping with a wrong date).
pub(crate) fn since_epoch() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
}
