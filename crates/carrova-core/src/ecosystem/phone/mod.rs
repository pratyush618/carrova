//! The phone's side of the phone features: which apps may forward
//! notifications, what is withheld from a note before it leaves the phone,
//! and the forwarder that gives notifications, SMS and calls to linked
//! devices.

mod apps;
pub(super) mod give;
mod otp;

pub use apps::NotifyApps;
