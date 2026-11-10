/** The receiving side of the fake port: open or connect, plan, approve, apply. */
import type { Blocker } from '../../bindings/Blocker';
import type { Checklist } from '../../bindings/Checklist';
import type { Event } from '../../bindings/Event';
import type { PlanOptions } from '../../bindings/PlanOptions';
import type { PlanReview } from '../../bindings/PlanReview';
import type { SessionId } from '../../bindings/SessionId';
import type { SessionOpened } from '../../bindings/SessionOpened';
import { bridgeError, raise } from '../reject';
import * as fx from './fixtures';
import { planUnder } from './planner';
import { CANCELLED, later, type Runtime } from './runtime';
import type { Scenario } from './scenario';

/** The passphrase the fake treats as wrong. */
export const WRONG_PASSPHRASE = 'wrong';

/** Shape of a code or `code@ip:port` ticket (`7-acid-lemon`). */
const TICKET = /^\d+-[a-z]+-[a-z]+(@\d{1,3}(\.\d{1,3}){3}:\d+)?$/;

/** The running app of the `blocked` scenario, as its blocker. */
const RUNNING_APP = { kind: 'running_app', app: 'neovim', name: 'Neovim' } satisfies Blocker;

/** Receiving-side flows over `runtime`. */
export class FakeReceiver {
  private approved = false;
  /** Options of the latest plan, and of the plan last approved (`JSON.stringify`). */
  private planned: PlanOptions | null = null;
  private approvedOptions = '';
  private blocked = false;
  /** Whether the user lifted the free-space check of the current pause. */
  private space = { ignored: false };
  /** Cancel flag of a `connect` still pairing. */
  private connecting: { cancelled: boolean } | null = null;

  constructor(
    private readonly runtime: Runtime,
    private readonly scenario: Scenario,
  ) {}

  async openArchive(passphrase: string): Promise<SessionOpened> {
    if (passphrase === WRONG_PASSPHRASE) {
      raise(
        bridgeError(
          'wrong_passphrase',
          'decryption failed: wrong passphrase or corrupted data',
          'Check the passphrase; if it is right, the file is damaged.',
        ),
      );
    }
    this.runtime.open(fx.importSession);
    await this.runtime.step(fx.importSession);
    return { session: fx.importSession };
  }

  async connect(code: string): Promise<SessionOpened> {
    if (!TICKET.test(code.trim())) {
      raise(bridgeError('bad_code', 'that is not a pairing code'));
    }
    if (this.connecting !== null) {
      raise(bridgeError('session', 'a pairing is already in progress'));
    }
    const pairing = { cancelled: false };
    this.connecting = pairing;
    try {
      this.runtime.open(fx.importSession);
      await this.runtime.step(fx.importSession, 3);
      if (this.scenario === 'wrong-code') {
        this.runtime.forget(fx.importSession);
        raise(
          bridgeError(
            'pairing_failed',
            'pairing failed: wrong code or tampered connection',
            'Check the code shown on the other machine now (a wrong attempt replaces it).',
          ),
        );
      }
      if (pairing.cancelled) {
        this.runtime.forget(fx.importSession);
        raise(CANCELLED);
      }
      this.runtime.engine(fx.importSession, { event: 'peer_connected', peer: fx.peerAddress });
      return { session: fx.importSession };
    } finally {
      this.connecting = null;
    }
  }

  cancelConnect(): void {
    if (this.connecting !== null) {
      this.connecting.cancelled = true;
    }
  }

  /** Re-plan the recording under the user's `options` (see `planUnder`). */
  plan(session: SessionId, options: PlanOptions): Promise<PlanReview> {
    return later(() => {
      this.runtime.slot(session);
      // Like the engine: a plan approved with the same options stays approved.
      this.planned = options;
      this.approved = this.approvedOptions === JSON.stringify(options);
      const running = this.scenario === 'blocked';
      const replanned = planUnder(fx.plan, options, { runningApp: running ? RUNNING_APP : null });
      const review: PlanReview = { ...replanned, approval: this.approved ? 'given' : 'needed' };
      this.blocked = review.plan.blockers.length > 0;
      if (!running) return review;
      const plan = {
        ...review.plan,
        running_apps: [RUNNING_APP.app],
        app_names: { ...review.plan.app_names, [RUNNING_APP.app]: RUNNING_APP.name },
      };
      return { ...review, plan };
    });
  }

  planOptions(session: SessionId): Promise<PlanOptions | null> {
    return later(() => {
      this.runtime.slot(session);
      return this.planned;
    });
  }

  approve(session: SessionId): Promise<void> {
    return later(() => {
      this.runtime.slot(session);
      if (this.blocked) {
        raise(
          bridgeError(
            'blocked',
            'plan is blocked: resolve the blockers first',
            'Resolve the blockers, deselect those items, or override where offered.',
          ),
        );
      }
      this.approved = true;
      this.approvedOptions = JSON.stringify(this.planned);
    });
  }

  apply(session: SessionId): Promise<Checklist> {
    return this.runtime.task(session, async () => {
      if (!this.approved) {
        raise(bridgeError('session', 'plan not approved'));
      }
      let applied = 0;
      let interrupted = false;
      // Like the engine: cancellable until apply starts writing, never after.
      let writing = false;
      for (const { event } of fx.importEvents.filter((e) => e.kind === 'engine')) {
        await (writing ? this.runtime.pause() : this.runtime.step(session));
        writing ||= event.event === 'state_changed' && event.to === 'applying';
        if (event.event === 'progress' && !interrupted) {
          interrupted = true;
          await this.interruption(session);
        }
        this.runtime.engine(session, event);
        if (event.event === 'action_applied' && ++applied === 3 && this.fails()) {
          await this.rollBack(session);
        }
      }
      return fx.checklist;
    });
  }

  /** Hold or continue the transfer: the engine announces both. */
  setTransferPaused(session: SessionId, paused: boolean): Promise<void> {
    return later(() => {
      this.runtime.slot(session);
      this.runtime.engine(
        session,
        paused
          ? { event: 'transfer_paused', reason: { kind: 'by_user' } }
          : { event: 'transfer_resumed' },
      );
    });
  }

  /** Continue a low-disk-space pause at once. */
  ignoreFreeSpace(session: SessionId): Promise<void> {
    return later(() => {
      this.runtime.slot(session);
      this.space.ignored = true;
    });
  }

  endReview(session: SessionId): Promise<void> {
    return later(() => {
      this.runtime.slot(session);
    });
  }

  private fails(): boolean {
    return this.scenario === 'apply-fails';
  }

  /** The `disk-full` pause or the `reconnect` drop, before the first progress. */
  private async interruption(session: SessionId): Promise<void> {
    const rt = this.runtime;
    if (this.scenario === 'disk-full') {
      const reason = {
        kind: 'low_disk_space',
        fs: '/home/ana/.local/state/carrova',
        available: 1_000_000,
        needed: 250_000_000,
      } as const;
      rt.engine(session, { event: 'transfer_paused', reason });
      const space = { ignored: false };
      this.space = space;
      // Space "frees up" after a while, or the user continues anyway.
      for (let waited = 0; waited < 8 && !space.ignored; waited++) {
        await rt.step(session);
      }
      rt.engine(session, { event: 'transfer_resumed' });
    } else if (this.scenario === 'reconnect') {
      rt.engine(session, { event: 'peer_lost', retrying: true });
      await rt.step(session, 3);
      rt.engine(session, { event: 'peer_connected', peer: fx.peerAddress });
    }
  }

  /** Undo what was applied and fail like the engine's automatic rollback. */
  private async rollBack(session: SessionId): Promise<never> {
    const steps: Event[] = [
      ...fx.rollback.restored.map((target) => ({
        event: 'rollback_step' as const,
        target,
        outcome: 'restored' as const,
      })),
      ...fx.rollback.removed.map((target) => ({
        event: 'rollback_step' as const,
        target,
        outcome: 'removed' as const,
      })),
    ];
    for (const step of steps) {
      await this.runtime.pause();
      this.runtime.engine(session, step);
    }
    this.runtime.engine(session, {
      event: 'state_changed',
      session,
      from: 'applying',
      to: 'rolled_back',
    });
    raise(
      bridgeError(
        'internal',
        'apply failed (handler gpg-keyring failed: keyring locked); changes were rolled back',
        'Nothing was changed: the apply was rolled back.',
      ),
    );
  }
}
