/**
 * The fake port's moving parts: the event bus, per-session bookkeeping
 * (cancel, a pending pairing question) and paced steps whose timing tests
 * control.
 */
import type { BridgeEvent } from '../../bindings/BridgeEvent';
import type { BridgeError } from '../../bindings/BridgeError';
import type { Event } from '../../bindings/Event';
import type { SessionId } from '../../bindings/SessionId';
import { toBridgeError } from '../guards';
import type { Unsubscribe } from '../port';
import { bridgeError, raise } from '../reject';

/** Waits `ms` milliseconds. */
export type Sleep = (ms: number) => Promise<void>;

/** Real-time sleep. */
export const realSleep: Sleep = (ms) =>
  new Promise((resolve) => {
    setTimeout(resolve, ms);
  });

interface SessionSlot {
  cancelled: boolean;
  /** Resolves the open "show a new code?" question. */
  answer?: (offerNewCode: boolean) => void;
}

/**
 * `compute`'s result as a promise; anything it throws becomes a rejection
 * (the port never throws synchronously).
 */
export function later<T>(compute: () => T): Promise<T> {
  return Promise.resolve().then(compute);
}

export const CANCELLED: BridgeError = bridgeError('cancelled', 'cancelled');

/** Event bus + sessions + pacing shared by the fake's flows. */
export class Runtime {
  private readonly listeners = new Set<(event: BridgeEvent) => void>();
  private readonly sessions = new Map<SessionId, SessionSlot>();

  constructor(
    private readonly stepMs: number,
    private readonly sleep: Sleep,
  ) {}

  subscribe(listener: (event: BridgeEvent) => void): Unsubscribe {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  emit(event: BridgeEvent): void {
    for (const listener of this.listeners) {
      listener(event);
    }
  }

  /** Emit an engine event of `session`. */
  engine(session: SessionId, event: Event): void {
    this.emit({ kind: 'engine', session, event });
  }

  /** Start (or restart, after resume) tracking `session`. */
  open(session: SessionId): void {
    this.sessions.set(session, { cancelled: false });
  }

  /** The slot of a live session, else reject with `session`. */
  slot(session: SessionId): SessionSlot {
    const slot = this.sessions.get(session);
    if (slot === undefined) {
      raise(bridgeError('session', 'no such session is open'));
    }
    return slot;
  }

  forget(session: SessionId): void {
    this.sessions.delete(session);
  }

  /** Cancel `session`'s running task; an open question is answered "no". */
  cancel(session: SessionId): void {
    const slot = this.slot(session);
    slot.cancelled = true;
    slot.answer?.(false);
  }

  /** Wait `steps` pacing steps, then reject if `session` was cancelled. */
  async step(session: SessionId, steps = 1): Promise<void> {
    await this.sleep(this.stepMs * steps);
    if (this.slot(session).cancelled) {
      raise(CANCELLED);
    }
  }

  /** Wait one pacing step, whatever happens to the session. */
  async pause(): Promise<void> {
    await this.sleep(this.stepMs);
  }

  /** Ask the user (via `answerPairing`) and wait for the answer. */
  question(session: SessionId): Promise<boolean> {
    const slot = this.slot(session);
    return new Promise((resolve) => {
      slot.answer = (offerNewCode) => {
        delete slot.answer;
        resolve(offerNewCode);
      };
    });
  }

  /** Answer `session`'s open question, else reject with `session`. */
  answer(session: SessionId, offerNewCode: boolean): void {
    const answer = this.slot(session).answer;
    if (answer === undefined) {
      raise(bridgeError('session', 'no pairing question is open for this session'));
    }
    answer(offerNewCode);
  }

  /**
   * Run a long task of `session`, reporting its end as `task_ended` like the
   * bridge does, with any rejection normalised.
   */
  async task<T>(session: SessionId, run: () => Promise<T>): Promise<T> {
    try {
      const result = await run();
      this.emit({ kind: 'task_ended', session, error: null });
      return result;
    } catch (reason: unknown) {
      const error = toBridgeError(reason);
      this.emit({ kind: 'task_ended', session, error });
      raise(error);
    }
  }
}
