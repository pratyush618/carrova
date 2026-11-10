//! [`RunBudget`]: what one plugin may use over a whole restore (or capture).

use std::time::Duration;

/// Fuel one plugin may burn over all files of one run (about five seconds
/// on a fast laptop).
pub const MAX_RUN_FUEL: u64 = 5 * crate::MAX_FUEL;
/// Log bytes one plugin may write over one run; the rest is dropped after
/// one "truncated" marker.
pub const MAX_RUN_LOG: usize = 16 * 1024;
/// Wall-clock time one plugin's tools may run over one run.
pub const MAX_RUN_TOOL_TIME: Duration = Duration::from_secs(60);

/// What is left of one plugin's allowance for one run: a restore (all its
/// files on the receiver) or a capture (all its files on the sender).
/// Callers keep one per plugin and run and pass it to every call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunBudget {
    pub(crate) fuel: u64,
    /// Log bytes left; `None` once the truncation marker was written.
    pub(crate) log: Option<usize>,
    pub(crate) tool_time: Duration,
}

impl RunBudget {
    /// A full budget: [`MAX_RUN_FUEL`], [`MAX_RUN_LOG`],
    /// [`MAX_RUN_TOOL_TIME`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            fuel: MAX_RUN_FUEL,
            log: Some(MAX_RUN_LOG),
            tool_time: MAX_RUN_TOOL_TIME,
        }
    }

    /// This budget with `fuel` left instead.
    #[must_use]
    pub const fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = fuel;
        self
    }

    /// This budget with `time` of tool running left instead.
    #[must_use]
    pub const fn with_tool_time(mut self, time: Duration) -> Self {
        self.tool_time = time;
        self
    }

    /// Fuel left.
    #[must_use]
    pub const fn fuel(&self) -> u64 {
        self.fuel
    }

    /// `line` as it may be logged: all of it while the budget lasts, then
    /// its fitting part with a truncation marker once, then nothing.
    pub(crate) fn take_log(&mut self, mut line: String) -> Option<String> {
        let left = self.log?;
        if line.len() <= left {
            self.log = Some(left - line.len());
        } else {
            line.truncate(line.floor_char_boundary(left));
            line.push_str("… [plugin log truncated]");
            self.log = None;
        }
        Some(line)
    }
}

impl Default for RunBudget {
    fn default() -> Self {
        Self::new()
    }
}
