import type { AppRef, Bytes, MoveRoute } from './model';

/**
 * Where a transfer is: moving, paused for space, held by the user, waiting
 * for the link, given up on the link (resumable), or the final apply.
 */
export type TransferPhase = 'moving' | 'paused' | 'held' | 'reconnecting' | 'lost' | 'applying';

/** Per-app progress, in the engine's order (small first). */
export interface TransferApp extends AppRef {
  readonly bytes: Bytes;
  readonly state: 'waiting' | 'moving' | 'done' | 'failed';
  /** Bytes arrived so far, for the app currently moving. */
  readonly bytesDone: Bytes;
}

/** One transfer as the progress screens show it (send live, receive progress). */
export interface TransferModel {
  readonly side: 'send' | 'receive';
  readonly phase: TransferPhase;
  readonly peer: string;
  readonly done: Bytes;
  readonly total: Bytes;
  /** From the measured rate while moving; `null` until it is known. */
  readonly secondsLeft: number | null;
  /** Measured bytes per second while moving; `null` otherwise. */
  readonly bytesPerSecond: number | null;
  /** The two computers, when known (the live card's ends). */
  readonly route: MoveRoute | null;
  readonly apps: readonly TransferApp[];
  /** Paused: bytes still needed on this disk. */
  readonly shortfall: Bytes;
  /** Paused: where that disk is mounted, when the engine knows. */
  readonly disk: string | null;
}

/** Optional intents are offered only when the engine supports them (no dead buttons). */
export interface TransferIntents {
  readonly onCancel: () => void;
  readonly onPause?: () => void;
  readonly onResume?: () => void;
  readonly onIgnoreSpace?: () => void;
  /** Reconnecting: try the link now instead of after the back-off. */
  readonly onTryNow?: () => void;
  /** Lost: pick the session up again (what arrived is kept). */
  readonly onResumeSession?: () => void;
  readonly onOpenFiles?: () => void;
  readonly onSeeArrived?: () => void;
}

/** Stopped part-way, with what arrived kept (disk full, held, link lost). */
export const interrupted = (phase: TransferPhase): boolean =>
  phase === 'paused' || phase === 'held' || phase === 'reconnecting' || phase === 'lost';

/**
 * The app a stopped transfer is paused at: the one moving, else the next to
 * move (the link dropped between apps). `null` while bytes flow.
 */
export function stoppedAt(model: TransferModel): string | null {
  if (!interrupted(model.phase)) return null;
  const at =
    model.apps.find((a) => a.state === 'moving') ?? model.apps.find((a) => a.state === 'waiting');
  return at?.id ?? null;
}
