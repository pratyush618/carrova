//! [`Failpoint`]: test-only fault injection into apply.

#[cfg(feature = "failpoints")]
use crate::error::CoreError;
use crate::error::Result;

/// Test-only fault injection into apply (feature `failpoints`): fail
/// once a number of actions completed. In every other build it is
/// zero-sized and its check compiles to nothing; only `failpoints`
/// builds can arm one ([`Self::after`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Failpoint {
    /// Fail once this many actions completed.
    #[cfg(feature = "failpoints")]
    after: Option<usize>,
}

impl Failpoint {
    /// No injected failure.
    pub const OFF: Self = Self {
        #[cfg(feature = "failpoints")]
        after: None,
    };

    /// Fail apply (and roll back) once `actions` actions completed.
    #[cfg(feature = "failpoints")]
    #[must_use]
    pub const fn after(actions: usize) -> Self {
        Self {
            after: Some(actions),
        }
    }

    /// [`CoreError::Injected`] once `done` actions completed, if this is
    /// the point to fail at.
    #[cfg(feature = "failpoints")]
    pub(super) fn check(self, done: usize) -> Result<()> {
        if self.after == Some(done) {
            return Err(CoreError::Injected("apply"));
        }
        Ok(())
    }

    /// Never fails: nothing can be injected outside `failpoints` builds.
    #[cfg(not(feature = "failpoints"))]
    #[inline]
    pub(super) const fn check(self, _done: usize) -> Result<()> {
        Ok(())
    }
}
