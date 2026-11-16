//! Phone features on both sides of a link. The phone's side: which apps
//! may forward notifications, what is withheld from a note before it
//! leaves the phone, and the forwarder that gives notifications, SMS and
//! calls to linked devices. The laptop's side: the in-memory mirror of
//! what a phone gives, and the user's settings for showing it.

mod apps;
pub(super) mod calls;
pub(super) mod give;
pub(super) mod mirror;
mod otp;
pub(super) mod settings;

pub use apps::NotifyApps;
pub use calls::CallView;
pub use settings::{PhoneSettings, RingHere};
