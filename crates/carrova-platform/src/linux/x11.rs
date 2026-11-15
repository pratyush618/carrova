//! Asking the X11 clipboard owner for one of its targets (`TARGETS`, a
//! password manager's hint): what `arboard` cannot do.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ConnectionExt, CreateWindowAux, EventMask, WindowClass,
};
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME};

use crate::PlatformError;

/// How long the owner may take to answer.
const REPLY_TIMEOUT: Duration = Duration::from_secs(1);
const POLL: Duration = Duration::from_millis(5);
/// Longest answer read (`TARGETS` and hints are tiny); in 32-bit words.
const MAX_WORDS: u32 = 1 << 14;

fn err(e: impl std::fmt::Display) -> PlatformError {
    PlatformError::Clipboard(e.to_string())
}

/// The target names the clipboard owner offers.
pub(super) fn targets() -> Result<HashSet<String>, PlatformError> {
    let (conn, reply) = request("TARGETS")?;
    let atoms: Vec<Atom> = reply.value32().ok_or_else(|| err("bad TARGETS"))?.collect();
    atoms
        .into_iter()
        .map(|atom| {
            let name = conn
                .get_atom_name(atom)
                .map_err(err)?
                .reply()
                .map_err(err)?;
            Ok(String::from_utf8_lossy(&name.name).into_owned())
        })
        .collect()
}

/// The clipboard content converted to `target`.
pub(super) fn selection(target: &str) -> Result<Vec<u8>, PlatformError> {
    Ok(request(target)?.1.value)
}

type Reply = x11rb::protocol::xproto::GetPropertyReply;

/// Convert the CLIPBOARD selection to `target` and read the answer.
fn request(target: &str) -> Result<(impl Connection, Reply), PlatformError> {
    let (conn, screen) = x11rb::connect(None).map_err(err)?;
    let atom = |name: &str| -> Result<Atom, PlatformError> {
        Ok(conn
            .intern_atom(false, name.as_bytes())
            .map_err(err)?
            .reply()
            .map_err(err)?
            .atom)
    };
    let (clipboard, target, property) = (atom("CLIPBOARD")?, atom(target)?, atom("CARROVA_SEL")?);
    let window = conn.generate_id().map_err(err)?;
    conn.create_window(
        COPY_DEPTH_FROM_PARENT,
        window,
        conn.setup().roots[screen].root,
        0,
        0,
        1,
        1,
        0,
        WindowClass::INPUT_ONLY,
        0,
        &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )
    .map_err(err)?;
    conn.convert_selection(window, clipboard, target, property, CURRENT_TIME)
        .map_err(err)?;
    conn.flush().map_err(err)?;

    let deadline = Instant::now() + REPLY_TIMEOUT;
    loop {
        match conn.poll_for_event().map_err(err)? {
            Some(Event::SelectionNotify(n)) if n.requestor == window => {
                if n.property == u32::from(AtomEnum::NONE) {
                    return Err(err("the clipboard owner refused the target"));
                }
                let reply = conn
                    .get_property(true, window, property, AtomEnum::ANY, 0, MAX_WORDS)
                    .map_err(err)?
                    .reply()
                    .map_err(err)?;
                return Ok((conn, reply));
            }
            Some(_) => {}
            None if Instant::now() >= deadline => return Err(err("the clipboard owner timed out")),
            None => std::thread::sleep(POLL),
        }
    }
}
