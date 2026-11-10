//! Receiver: which senders nearby are offering a code ([`browse_nearby`]).

use carrova_platform::Platform;
use carrova_transport::{Nearby, browse};

use super::{BROWSE_WINDOW, clean_device_name};
use crate::error::Result;

/// Senders on the LAN offering a pairing code, collected for
/// [`BROWSE_WINDOW`]. Device names only, sanitised like the receiver's
/// [`Inventory::device_name`](super::Inventory::device_name) (names that
/// sanitise to nothing are left out); the user still types the code.
///
/// # Errors
///
/// Discovery could not start.
pub fn browse_nearby(platform: &dyn Platform) -> Result<Vec<Nearby>> {
    Ok(browse(platform, BROWSE_WINDOW)?
        .into_iter()
        .filter_map(|n| clean_device_name(&n.device_name).map(|device_name| Nearby { device_name }))
        .collect())
}
