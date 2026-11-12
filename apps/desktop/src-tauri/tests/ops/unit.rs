//! The operations driven directly over [`AppState`](crate::state::AppState)
//! on fake machines, without a window.
#![allow(clippy::unwrap_used)]

mod archive;
mod background;
mod contract;
mod inbox;
mod record;
mod selection;
mod services;
