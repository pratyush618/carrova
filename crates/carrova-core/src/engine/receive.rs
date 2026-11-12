//! LAN receiver: pairing, resume and the chunk pull that stages an
//! approved plan.

use std::sync::Arc;

use carrova_crypto::pairing::ResumeSecret;
use carrova_manifest::{Blake3Hash, Manifest};
use carrova_transport::{ConnectOptions, Connection, FailedChunk, Target, Ticket, connect};

use super::import::{ImportFrom, Importer};
use super::remember::KnownPeer;
use super::resume::ResumePoint;
use super::session::Session;
use super::transfer::{Disk, Transfer, next_batch};
use super::{Engine, sessions};
use crate::cancel::Cancel;
use crate::capture::MANIFEST_FILE;
use crate::error::{CoreError, Result, SessionProblem};
use crate::lan::link::TransportDialer;
use crate::lan::{
    self, FINAL_NOTICE_WINDOW, Inventory, LanMsg, Link, PAIR_TIMEOUT, RECONNECT_WINDOW,
    REPLY_TIMEOUT, ReceiverFailure, ReceiverStage,
};
use crate::plan::Plan;
use crate::session::{Event, Role, SessionId, State};
use crate::stage::Staged;
use crate::store::{self, PLAN_FILE, Source, read_file, write_atomic};
use crate::transfer_control::TransferControl;

impl Importer {
    /// Pair with the sender named by `ticket`, send this machine's
    /// inventory and receive the manifest: a new LAN import session in
    /// state `Selected`. Plan, approve and apply as for an archive; staging
    /// pulls chunks from the sender and reconnects automatically.
    ///
    /// The manifest is decoded with [`Manifest::from_cbor`], i.e. the same
    /// size cap and [`Manifest::validate`] checks as an archive's (counts,
    /// text, paths, modes, chunk lengths within
    /// `1..=`[`carrova_manifest::MAX_CHUNK_LEN`] and one length per hash, so
    /// the sender cannot size the receiver's chunk buffers), and
    /// receiver-side sensitivity applies when planning. Only Carrova's own session directory is written
    /// (`manifest.cbor`, the `600` resume secret, the record and log).
    ///
    /// Not cancellable; see [`Self::connect_with`].
    pub fn connect(engine: Engine, ticket: &Ticket) -> Result<Self> {
        Self::connect_with(engine, ticket, Cancel::default())
    }

    /// [`Self::connect`], stopped by `cancel` (which becomes this
    /// importer's [`Self::canceller`]) from another thread.
    ///
    /// A cancel during discovery, pairing or the manifest exchange ends
    /// the call with [`CoreError::Cancelled`] within the transport's cancel
    /// polling interval. No session exists until the manifest has arrived,
    /// so nothing is left behind; a sender already paired is told
    /// ([`lan::AbortReason::Cancelled`]).
    pub fn connect_with(engine: Engine, ticket: &Ticket, cancel: Cancel) -> Result<Self> {
        let me = engine.device_key();
        let mut conn = connect(
            Target::Pairing(ticket),
            PAIR_TIMEOUT,
            &ConnectOptions {
                platform: engine.platform().clone(),
                device: me.as_ref().map(|(key, _)| Arc::clone(key)),
                cancel: Some(cancel.signal()),
            },
        )?;
        let bytes = match exchange_manifest(&engine, &mut conn) {
            Ok(bytes) => bytes,
            Err(CoreError::Cancelled) => {
                lan::farewell_cancelled(conn);
                return Err(CoreError::Cancelled);
            }
            Err(e) => return Err(e),
        };
        let manifest = Manifest::from_cbor(bytes.as_slice())?;
        let s = Session::create(
            engine,
            Role::Import,
            State::Selected,
            Some(Source::Lan {
                peer_cert_hash: Some(conn.peer_cert_hash()),
                manifest_fingerprint: Blake3Hash::of(&bytes),
                peer_name: manifest.source.device_name.clone(),
            }),
        )?;
        write_atomic(s.platform(), &s.dir, MANIFEST_FILE, &bytes)?;
        store::write_resume_secret(s.platform(), &s.dir, conn.resume_secret())?;
        tracing::info!(session = %s.record.id, "paired with the sender");
        s.emit(Event::PeerConnected {
            peer: conn.peer_addr(),
        });
        let known_peer = me.and_then(|(_, epoch)| KnownPeer::of(&conn, epoch));
        let dialer = TransportDialer::boxed(s.engine.platform().clone(), &cancel);
        let control = TransferControl::default();
        let link = Link::paired(conn, dialer, cancel.clone(), control.clone());
        let mut imp = Self::started(s, ImportFrom::Lan(Some(Box::new(link))), manifest, cancel)?;
        imp.control = control;
        imp.known_peer = known_peer;
        Ok(imp)
    }

    /// Resume an interrupted LAN import (`id`, or the newest resumable
    /// one: [`CoreError::NothingToResume`] if none): reconnect to the sender with the
    /// session's resume secret and continue from the persisted state,
    /// keeping chunks already staged. States afterwards match
    /// [`Self::resume_archive`]:
    /// - `Selected`: plan again.
    /// - `Planned`, `Transferring`, `Staged`: the plan is recomputed
    ///   (`Planned`, approval kept if unchanged); `apply` then fetches only
    ///   chunks not yet checkpointed in staging.
    /// - `Applying`: rolled back first (no sender needed), ends
    ///   `RolledBack`/`Failed`; the sender is told (best effort).
    /// - `Done`, `RolledBack`, `Failed`: nothing to resume (error).
    ///
    /// Not cancellable while reconnecting; see [`Self::resume_lan_with`].
    pub fn resume_lan(engine: Engine, id: Option<&SessionId>) -> Result<Self> {
        Self::resume_lan_with(engine, id, Cancel::default())
    }

    /// [`Self::resume_lan`], stopped by `cancel` (which becomes this
    /// importer's [`Self::canceller`]) from another thread.
    ///
    /// A cancel while reconnecting ends the call with
    /// [`CoreError::Cancelled`] within the transport's cancel polling
    /// interval. The session is left as any failed reconnect leaves it:
    /// re-planned (`Planned`, approval kept if the plan is unchanged) with
    /// its staging and resume secret, so it can be resumed again. Crash
    /// recovery of an interrupted apply is never cancelled.
    pub fn resume_lan_with(engine: Engine, id: Option<&SessionId>, cancel: Cancel) -> Result<Self> {
        let id = &sessions::lan_resume_target(&engine, id, Role::Import)?;
        let mut s = Session::load(engine, id)?;
        if s.record.role != Role::Import {
            return Err(s.problem(SessionProblem::SendNotReceive));
        }
        let Some(Source::Lan {
            manifest_fingerprint,
            ..
        }) = s.record.source.clone()
        else {
            return Err(s.problem(SessionProblem::NotLanImport));
        };
        let secret = store::read_resume_secret(s.platform(), &s.dir)?;
        let old_plan: Option<Plan> = s.read_json(PLAN_FILE)?;
        match ResumePoint::of(&s)? {
            ResumePoint::Recover => {
                s.recover_interrupted_apply()?;
                let manifest = read_lan_manifest(&s, &manifest_fingerprint).ok();
                let mut imp = Self::finished(s, ImportFrom::Lan(None), old_plan, manifest, cancel);
                if let Some(secret) = secret {
                    let link = imp.link(secret);
                    imp.from = ImportFrom::Lan(Some(Box::new(link)));
                    imp.notify_sender(&ReceiverFailure::ApplyInterrupted.into());
                }
                Ok(imp)
            }
            ResumePoint::Replan(state) => {
                let secret = secret.ok_or_else(|| s.problem(SessionProblem::NoResumeSecret))?;
                let manifest = read_lan_manifest(&s, &manifest_fingerprint);
                let manifest = s.unless_changed(manifest)?;
                let mut imp = Self::resumed(s, ImportFrom::Lan(None), manifest, cancel);
                imp.replan_after_restart(state, old_plan)?;
                let mut link = imp.link(secret);
                link.reconnect(imp.lan_stage(), RECONNECT_WINDOW, imp.s.sink())?;
                imp.from = ImportFrom::Lan(Some(Box::new(link)));
                Ok(imp)
            }
        }
    }

    /// End a dry run: a LAN sender is told that nothing will be
    /// transferred (it finishes cleanly) and the resume secret is wiped,
    /// so the session cannot be resumed. The session stays at its plan;
    /// nothing outside Carrova's session dir was written. No-op for
    /// archive imports.
    pub fn end_dry_run(&mut self) -> Result<()> {
        if !matches!(self.from, ImportFrom::Lan(_)) {
            return Ok(());
        }
        self.notify_sender(&LanMsg::Abort {
            reason: lan::AbortReason::DryRun,
        });
        store::wipe_resume_secret(self.s.platform(), &self.s.dir)
    }

    /// A link (not yet connected) to the sender proven by `secret`, on this
    /// engine's platform, stopped by this session's cancel.
    fn link(&self, secret: ResumeSecret) -> Link {
        let dialer = TransportDialer::boxed(self.s.engine.platform().clone(), &self.cancel);
        Link::detached(secret, dialer, self.cancel.clone(), self.control.clone())
    }

    /// Stage at which the sender resumes with us.
    fn lan_stage(&self) -> ReceiverStage {
        if self.s.record.approved {
            ReceiverStage::Transferring
        } else {
            ReceiverStage::Planning
        }
    }

    /// Final best-effort message to a LAN sender (checklist or abort);
    /// the link is closed afterwards. No-op for archives or when already
    /// notified.
    pub(super) fn notify_sender(&mut self, msg: &LanMsg) {
        if let ImportFrom::Lan(link) = &mut self.from
            && let Some(link) = link.take()
        {
            link.notify(msg, FINAL_NOTICE_WINDOW, self.s.sink());
        }
    }

    /// Pull every chunk `plan` still needs from the sender into staging
    /// (kept across reconnects and restarts) and verify it.
    ///
    /// Batches of [`TRANSFER_BATCH_BYTES`] in [`lan::transfer_order`],
    /// minus chunks already checkpointed; [`lan::check_space`] before
    /// each batch and a checkpoint after it. Low disk space pauses
    /// ([`Event::TransferPaused`]) until space frees up; a dropped
    /// connection is re-established with the resume secret. The user may
    /// pause between batches or lift the free-space guard
    /// ([`Self::transfer_control`]). Chunks the
    /// sender cannot supply fail the transfer with
    /// [`CoreError::TransferIncomplete`] after everything else arrived.
    /// A cancel (either side) stops it after checkpointing.
    pub(super) fn stage_lan(&mut self, plan: &Plan) -> Result<Staged> {
        let Self {
            s,
            from,
            manifest,
            cancel,
            control,
            ..
        } = self;
        let manifest = manifest.open(s.record.state, State::Transferring)?;
        let ImportFrom::Lan(Some(link)) = from else {
            return Err(CoreError::LanEnded);
        };
        let staging = s.staging();
        let disk = Disk::new(
            s.platform(),
            &staging,
            s.sink(),
            s.store.root(),
            cancel,
            control,
        );
        let order = lan::transfer_order(plan, manifest)?;
        let (mut transfer, mut queue) = Transfer::open(disk, plan, manifest, &order)?;
        let mut failed: Vec<FailedChunk> = Vec::new();
        while !queue.is_empty() {
            cancel.check()?;
            control.hold_while_paused(cancel, s.sink())?;
            failed.extend(transfer.fetch_batch(link, next_batch(&queue))?);
            queue = transfer.missing(&queue, &failed);
        }
        transfer.finish(&failed)
    }
}

/// Right after pairing: send this machine's inventory and receive the
/// sender's manifest (CBOR, not yet decoded).
fn exchange_manifest(engine: &Engine, conn: &mut Connection) -> Result<Vec<u8>> {
    conn.send(&LanMsg::Inventory(Inventory::collect(
        &**engine.platform(),
        engine.recipes(),
    )))?;
    lan::recv_manifest(conn, REPLY_TIMEOUT)
}

/// The manifest received at pairing (`manifest.cbor`), checked against the
/// session's `fingerprint`.
fn read_lan_manifest(s: &Session, fingerprint: &Blake3Hash) -> Result<Manifest> {
    let bytes = read_file(s.platform(), &s.dir.join(MANIFEST_FILE))?
        .ok_or(CoreError::MissingFile(MANIFEST_FILE))?;
    if Blake3Hash::of(&bytes) != *fingerprint {
        return Err(CoreError::Verification(
            "manifest changed since pairing".into(),
        ));
    }
    Ok(Manifest::from_cbor(bytes.as_slice())?)
}
