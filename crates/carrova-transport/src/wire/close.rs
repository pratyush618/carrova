//! Application close and stream reset codes.

use quinn::VarInt;

/// Application close codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseCode {
    /// Orderly close.
    Normal,
    /// Key confirmation or resume proof failed.
    PairingFailed,
    /// The peer broke the protocol (e.g. requested a chunk outside the
    /// manifest).
    Protocol,
    /// The sender is not offering what the receiver asked for.
    Rejected,
    /// The user on this side cancelled the session.
    Cancelled,
}

impl CloseCode {
    const ALL: [Self; 5] = [
        Self::Normal,
        Self::PairingFailed,
        Self::Protocol,
        Self::Rejected,
        Self::Cancelled,
    ];

    pub(crate) const fn to_wire(self) -> VarInt {
        VarInt::from_u32(match self {
            Self::Normal => 0,
            Self::PairingFailed => 1,
            Self::Protocol => 2,
            Self::Rejected => 3,
            Self::Cancelled => 4,
        })
    }

    pub(crate) fn from_wire(code: VarInt) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.to_wire() == code)
    }

    /// Close `conn` with this code.
    pub(crate) fn close(self, conn: &quinn::Connection) {
        conn.close(self.to_wire(), b"");
    }
}

/// Stream reset / stop-sending code when a fetch is cancelled.
pub(crate) const STREAM_CANCELLED: VarInt = VarInt::from_u32(0);

#[cfg(test)]
#[path = "../../tests/wire/close/unit.rs"]
mod tests;
