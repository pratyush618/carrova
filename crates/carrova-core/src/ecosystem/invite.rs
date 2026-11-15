//! The pairing invite a computer shows as a QR code (ADR 0002 §2):
//! `carrova://pair?v=1&t=<ticket>&k=<base32 key>`.

use std::str::FromStr;

use carrova_crypto::identity::{DEVICE_PUBLIC_LEN, DevicePublic};
use carrova_transport::Ticket;
use data_encoding::BASE32_NOPAD;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

use crate::error::{CoreError, Result};

const PREFIX: &str = "carrova://pair?";
const VERSION: &str = "1";
/// Everything but the characters a pairing code is made of.
const TICKET_SET: &AsciiSet = &NON_ALPHANUMERIC.remove(b'-').remove(b'.');

/// A scanned or shown invite: where to connect and whose key to expect.
///
/// Holds the pairing code: never logged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairInvite {
    /// Code and address of the computer waiting to pair.
    pub ticket: Ticket,
    /// The key that computer must prove it holds.
    pub key: DevicePublic,
}

impl PairInvite {
    /// The `carrova://pair?...` text for the QR code.
    #[must_use]
    pub fn to_uri(&self) -> String {
        let ticket = self.ticket.to_string();
        format!(
            "{PREFIX}v={VERSION}&t={}&k={}",
            utf8_percent_encode(&ticket, TICKET_SET),
            BASE32_NOPAD.encode(self.key.as_bytes()),
        )
    }
}

impl FromStr for PairInvite {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self> {
        let invalid = CoreError::InvalidInvite;
        let query = s
            .trim()
            .strip_prefix(PREFIX)
            .ok_or(invalid("not a carrova pairing link"))?;
        let mut parts = query
            .split('&')
            .map(|p| p.split_once('=').unwrap_or((p, "")));
        let (Some(("v", v)), Some(("t", t)), Some(("k", k)), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid("expected v, t and k"));
        };
        if v != VERSION {
            return Err(invalid("unsupported invite version"));
        }
        let t = percent_decode_str(t)
            .decode_utf8()
            .map_err(|_| invalid("bad ticket"))?;
        let ticket = t.parse().map_err(|_| invalid("bad ticket"))?;
        let raw: [u8; DEVICE_PUBLIC_LEN] = BASE32_NOPAD
            .decode(k.as_bytes())
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or(invalid("bad key"))?;
        let key = DevicePublic::from_bytes(&raw).map_err(|_| invalid("bad key"))?;
        Ok(Self { ticket, key })
    }
}
