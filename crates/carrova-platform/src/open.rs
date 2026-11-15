//! Checks before handing a URL or path to the OS's default handler.

use std::path::Path;

use crate::PlatformError;

#[cfg(test)]
#[path = "../tests/open/unit.rs"]
mod tests;

/// `url` if it is `http(s)`: anything else could start any program.
pub(crate) fn checked_url(url: &str) -> Result<&str, PlatformError> {
    let web = ["http://", "https://"].iter().any(|scheme| {
        url.get(..scheme.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
    });
    if web {
        Ok(url)
    } else {
        Err(PlatformError::InvalidInput(
            "only http(s) URLs may be opened",
        ))
    }
}

/// `path` if absolute: a relative one could read as an option or a URL.
pub(crate) fn checked_path(path: &Path) -> Result<&Path, PlatformError> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(PlatformError::InvalidInput(
            "only absolute paths may be opened",
        ))
    }
}
