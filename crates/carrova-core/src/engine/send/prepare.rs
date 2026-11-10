//! The sender's way into a LAN send: prepare one from a selecting
//! [`Exporter`], re-arm it after a send that never paired, or resume an
//! interrupted one.

use carrova_manifest::Blake3Hash;

use super::SendHandle;
use crate::cancel::Cancel;
use crate::capture::{self, Capture};
use crate::engine::session::Session;
use crate::engine::{Engine, Exporter, sessions};
use crate::error::{CoreError, Result, SessionProblem};
use crate::session::{Role, SessionId, State};
use crate::store::{self, Source};

impl Exporter {
    /// Prepare a LAN send from state `Selected` (state `Transferring`):
    /// capture, persist `manifest.cbor` + `capture.cbor` (so the send
    /// survives a restart), generate a code and bind + advertise a
    /// listener. Nothing leaves this machine until a receiver pairs in
    /// [`SendHandle::run`]. The handle shares [`Self::canceller`].
    pub fn prepare_send(&mut self) -> Result<SendHandle> {
        let selection = self.selected()?;
        self.s.transition(State::Transferring)?;
        let cancel = self.cancel.clone();
        self.s.failing(|s| {
            let platform = s.platform();
            let cap = capture::capture_with_plugins(
                platform,
                s.engine.recipes(),
                s.engine.plugins(),
                &selection,
                s.sink(),
                &cancel,
            )?;
            let manifest = cap.persist(platform, &s.dir)?;
            s.record.source = Some(Source::Lan {
                peer_cert_hash: None,
                manifest_fingerprint: Blake3Hash::of(&manifest),
                peer_name: None,
            });
            s.save()?;
            let session = Session::load(s.engine.clone(), s.id())?;
            SendHandle::listen(session, cap, manifest, None, cancel)
        })
    }

    /// After a send this exporter prepared ([`Self::prepare_send`]) has
    /// run: reload the session and, if the send ended before any receiver
    /// paired (back at `Selected`, see [`SendHandle::run`]), re-arm the
    /// exporter's cancel — the cancel that stopped that send is spent — so
    /// [`Self::prepare_send`] can offer a fresh code. Returns whether a new
    /// send can be prepared. Call it only once the send's `run` returned.
    pub fn reopen_after_send(&mut self) -> Result<bool> {
        let id = self.s.id().clone();
        self.s = Session::load(self.s.engine.clone(), &id)?;
        let reopened = self.s.record.state == State::Selected && self.selection.usable().is_some();
        if reopened {
            self.cancel.reset();
        }
        Ok(reopened)
    }

    /// Resume an interrupted LAN send (state `Transferring`, already
    /// paired; `id`, or the newest such send:
    /// [`CoreError::NothingToResume`] if none) from its persisted capture: listens for the paired receiver
    /// to reconnect with the resume secret; there is no new code
    /// ([`SendHandle::ticket`] is `None`).
    pub fn resume_send(engine: Engine, id: Option<&SessionId>) -> Result<SendHandle> {
        let id = &sessions::lan_resume_target(&engine, id, Role::Export)?;
        let s = Session::load(engine, id)?;
        let Some(Source::Lan {
            manifest_fingerprint,
            ..
        }) = s.record.source.clone()
        else {
            return Err(s.problem(SessionProblem::NotLanSend));
        };
        if s.record.role != Role::Export || s.record.state != State::Transferring {
            return Err(s.problem(SessionProblem::NotInterruptedSend));
        }
        let secret = store::read_resume_secret(s.platform(), &s.dir)?
            .ok_or_else(|| s.problem(SessionProblem::NeverPaired))?;
        let cap = Capture::load(s.platform(), &s.dir)?;
        let manifest = cap.manifest_cbor()?;
        if Blake3Hash::of(&manifest) != manifest_fingerprint {
            return Err(CoreError::Verification(
                "manifest changed since pairing".into(),
            ));
        }
        SendHandle::listen(s, cap, manifest, Some(secret), Cancel::default())
    }
}
