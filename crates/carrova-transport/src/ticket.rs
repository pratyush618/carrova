//! Fallback ticket: the code plus the sender's address, for networks where
//! discovery does not work.

use std::fmt;
use std::net::SocketAddrV4;
use std::str::FromStr;

use carrova_crypto::pairing::PairingCode;

use crate::error::{Result, TransportError};

/// What the receiver types: `<code>` or `<code>@<ipv4>:<port>`.
///
/// `Display` renders the canonical form for showing to the user; it holds
/// the pairing code, so it is never logged. `Debug` redacts the code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    /// Pairing code.
    pub code: PairingCode,
    /// Sender address; `None` means "find it by discovery".
    pub addr: Option<SocketAddrV4>,
}

impl fmt::Display for Ticket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.addr {
            Some(addr) => write!(f, "{}@{addr}", self.code),
            None => write!(f, "{}", self.code),
        }
    }
}

impl FromStr for Ticket {
    type Err = TransportError;

    fn from_str(s: &str) -> Result<Self> {
        let s = s.trim();
        let (code, addr) =
            match s.split_once('@') {
                Some((code, addr)) => (
                    code,
                    Some(addr.trim().parse::<SocketAddrV4>().map_err(|_| {
                        TransportError::InvalidTicket("expected <code>@<ipv4>:<port>")
                    })?),
                ),
                None => (s, None),
            };
        Ok(Self {
            code: code.parse()?,
            addr,
        })
    }
}
