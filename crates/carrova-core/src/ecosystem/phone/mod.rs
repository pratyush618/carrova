//! Phone notification forwarding, computer-side policy: which apps may
//! forward and what is withheld from a note before it leaves the phone.

mod apps;
mod otp;

pub use apps::NotifyApps;
