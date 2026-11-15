//! Files whose device left while they were being sent, kept by offer id so
//! the user can send them again (spec "Interrupted transfers show 'Device
//! left' with Retry"). In memory only; paths are never logged.

use std::collections::VecDeque;
use std::path::PathBuf;

use carrova_crypto::identity::DeviceId;

/// Files kept at most; the oldest goes first.
const KEPT: usize = 16;

/// The files that can be sent again, oldest first.
#[derive(Default)]
pub(super) struct Retries(VecDeque<(u64, DeviceId, PathBuf)>);

impl Retries {
    /// Keep file `id` (`path`, to `device`) for a retry.
    pub(super) fn keep(&mut self, id: u64, device: DeviceId, path: PathBuf) {
        if self.0.len() == KEPT {
            self.0.pop_front();
        }
        self.0.push_back((id, device, path));
    }

    /// Take file `id` out: where it went and its path.
    pub(super) fn take(&mut self, id: u64) -> Option<(DeviceId, PathBuf)> {
        let at = self.0.iter().position(|(i, ..)| *i == id)?;
        self.0.remove(at).map(|(_, device, path)| (device, path))
    }
}
