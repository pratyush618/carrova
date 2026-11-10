//! SPAKE2 exchange, session keys and key confirmation.

use std::fmt;

use hkdf::Hkdf;
use hmac::Mac;
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Spake2};
use zeroize::Zeroizing;

use super::keys::MacKey;
use super::{
    CONFIRMATION_LEN, ChannelBinding, HKDF_SALT, INFO_CONFIRM, INFO_RESUME, PairingCode,
    PairingError, RECEIVER_ID, Result, ResumeSecret, SENDER_ID,
};

/// Which side of the pairing this process is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PakeRole {
    /// Shows the code; SPAKE2 side A; confirms first.
    Sender,
    /// Types the code; SPAKE2 side B; confirms second.
    Receiver,
}

impl PakeRole {
    /// The other side.
    #[must_use]
    pub const fn peer(self) -> Self {
        match self {
            Self::Sender => Self::Receiver,
            Self::Receiver => Self::Sender,
        }
    }

    /// SPAKE2 identity, also the role prefix of every confirmation MAC.
    #[must_use]
    pub const fn identity(self) -> &'static [u8] {
        match self {
            Self::Sender => SENDER_ID,
            Self::Receiver => RECEIVER_ID,
        }
    }
}

/// One SPAKE2 message (public; sent to the peer as-is).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PakeMessage(Vec<u8>);

impl PakeMessage {
    /// Wrap bytes received from the peer (validated by [`Pake::finish`]).
    #[must_use]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Bytes to send.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Owned bytes to send.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// In-progress SPAKE2 exchange (one per pairing attempt; consumed by
/// [`Pake::finish`]).
pub struct Pake {
    role: PakeRole,
    state: Spake2<Ed25519Group>,
}

impl Pake {
    /// Start SPAKE2 for `role` with `code` as the password. Send the returned
    /// message to the peer. Uses the OS RNG.
    #[must_use]
    pub fn start(code: &PairingCode, role: PakeRole) -> (Self, PakeMessage) {
        let password = code.password();
        let a = Identity::new(SENDER_ID);
        let b = Identity::new(RECEIVER_ID);
        let (state, msg) = match role {
            PakeRole::Sender => Spake2::<Ed25519Group>::start_a(&password, &a, &b),
            PakeRole::Receiver => Spake2::<Ed25519Group>::start_b(&password, &a, &b),
        };
        (Self { role, state }, PakeMessage(msg))
    }

    /// This side's role.
    #[must_use]
    pub fn role(&self) -> PakeRole {
        self.role
    }

    /// Finish with the peer's message. Succeeds even for a wrong code (the
    /// keys then differ); only key confirmation detects that.
    pub fn finish(self, peer: &PakeMessage) -> Result<SessionKeys> {
        let key = Zeroizing::new(
            self.state
                .finish(peer.as_bytes())
                .map_err(|_| PairingError::InvalidPakeMessage)?,
        );
        let hk = Hkdf::<Sha256>::new(Some(HKDF_SALT), &key);
        let expand = |info: &[u8]| -> Result<Zeroizing<[u8; 32]>> {
            let mut out = Zeroizing::new([0u8; 32]);
            hk.expand(info, &mut *out)
                .map_err(|_| PairingError::KeySchedule)?;
            Ok(out)
        };
        let confirm = expand(INFO_CONFIRM)?;
        let resume = expand(INFO_RESUME)?;
        Ok(SessionKeys {
            confirm: MacKey::new(&confirm),
            resume: ResumeSecret::from_key(resume),
        })
    }
}

impl fmt::Debug for Pake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pake")
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}

/// Keys from a finished SPAKE2 exchange.
pub struct SessionKeys {
    pub(super) confirm: MacKey,
    pub(super) resume: ResumeSecret,
}

impl SessionKeys {
    /// This side's key confirmation over `binding`.
    pub fn confirm(&self, role: PakeRole, binding: &ChannelBinding) -> Result<Confirmation> {
        Ok(Confirmation(
            self.confirm
                .role_mac(role, binding)?
                .finalize()
                .into_bytes()
                .into(),
        ))
    }

    /// Check the peer's confirmation in constant time.
    pub fn verify(
        &self,
        peer_role: PakeRole,
        binding: &ChannelBinding,
        confirmation: &Confirmation,
    ) -> Result<()> {
        self.confirm
            .role_mac(peer_role, binding)?
            .verify_slice(&confirmation.0)
            .map_err(|_| PairingError::ConfirmationFailed)
    }

    /// Per-session resume secret (persist `0600`, delete when the session
    /// closes).
    #[must_use]
    pub fn resume_secret(&self) -> ResumeSecret {
        self.resume.clone()
    }
}

impl fmt::Debug for SessionKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionKeys(<redacted>)")
    }
}

/// A key-confirmation or resume proof MAC (public; sent to the peer).
#[derive(Debug, Clone, Copy)]
pub struct Confirmation(pub [u8; CONFIRMATION_LEN]);
