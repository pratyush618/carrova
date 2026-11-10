//! Argon2id passphrase key derivation.

use zeroize::Zeroizing;

use crate::{CryptoError, Passphrase, Result};

/// Salt length in bytes.
pub const SALT_LEN: usize = 16;

/// Argon2id parameters, stored in the archive header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub m_kib: u32,
    /// Iterations.
    pub t: u32,
    /// Parallelism (lanes).
    pub p: u32,
}

impl KdfParams {
    /// Default for new archives: m = 256 MiB, t = 3, p = 1.
    pub const DEFAULT: Self = Self {
        m_kib: 256 * 1024,
        t: 3,
        p: 1,
    };
    /// Weakest parameters a user may choose when *writing* (m = 64 MiB, t = 2).
    pub const FLOOR: Self = Self {
        m_kib: 64 * 1024,
        t: 2,
        p: 1,
    };
    /// Strongest parameters a reader accepts (m = 1 GiB, t = 10, p = 4).
    pub const CEILING: Self = Self {
        m_kib: 1024 * 1024,
        t: 10,
        p: 4,
    };

    /// Check bounds for **reading**: within [`Self::CEILING`] and above
    /// Argon2's own minimums. Writers additionally call
    /// [`Self::validate_for_write`].
    pub fn validate(&self) -> Result<()> {
        let bad = |m| Err(CryptoError::InvalidParams(m));
        if self.t == 0 {
            return bad("t must be at least 1");
        }
        if self.p == 0 {
            return bad("p must be at least 1");
        }
        if self.m_kib > Self::CEILING.m_kib {
            return bad("memory cost above 1 GiB");
        }
        if self.t > Self::CEILING.t {
            return bad("iterations above 10");
        }
        if self.p > Self::CEILING.p {
            return bad("parallelism above 4");
        }
        // Argon2 requires at least 8 KiB per lane (and an absolute minimum).
        if self.m_kib < argon2::Params::MIN_M_COST || self.m_kib < 8 * self.p {
            return bad("memory cost below Argon2 minimum (8 KiB per lane)");
        }
        Ok(())
    }

    /// [`Self::validate`] plus at least [`Self::FLOOR`].
    pub fn validate_for_write(&self) -> Result<()> {
        self.validate()?;
        if self.m_kib < Self::FLOOR.m_kib {
            return Err(CryptoError::InvalidParams("memory cost below 64 MiB"));
        }
        if self.t < Self::FLOOR.t {
            return Err(CryptoError::InvalidParams("iterations below 2"));
        }
        Ok(())
    }
}

/// Random per-archive salt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Salt(pub [u8; SALT_LEN]);

impl Salt {
    /// Fresh salt from the OS RNG.
    pub fn random() -> Result<Self> {
        let mut b = [0u8; SALT_LEN];
        getrandom::fill(&mut b).map_err(|_| CryptoError::Rng)?;
        Ok(Self(b))
    }
}

/// 256-bit symmetric key. Zeroized on drop; `Debug` prints `ArchiveKey(***)`.
pub struct ArchiveKey(pub(crate) Zeroizing<[u8; 32]>);

impl std::fmt::Debug for ArchiveKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ArchiveKey(***)")
    }
}

/// Derive the archive key with Argon2id (v0x13). Validates `params` for
/// reading ([`KdfParams::validate`]); writers must call
/// [`KdfParams::validate_for_write`] themselves.
pub fn derive_key(pass: &Passphrase, salt: &Salt, params: KdfParams) -> Result<ArchiveKey> {
    params.validate()?;
    let p = argon2::Params::new(params.m_kib, params.t, params.p, Some(32))
        .map_err(|_| CryptoError::InvalidParams("rejected by Argon2"))?;
    let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, p);
    let mut out = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(pass.expose().as_bytes(), &salt.0, out.as_mut_slice())
        .map_err(|_| CryptoError::Kdf)?;
    Ok(ArchiveKey(out))
}

// Tests use small `KdfParams` that pass `validate` (the read side has no floor).
#[cfg(test)]
#[path = "../tests/kdf/unit.rs"]
mod tests;
