import type { AppRef, Bytes } from './model';

/**
 * Where a transfer is: moving, paused for space, held by the user, waiting
 * for the link, or the final apply.
 */
export type TransferPhase = 'moving' | 'paused' | 'held' | 'reconnecting' | 'applying';

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
  readonly secondsLeft: number | null;
  readonly apps: readonly TransferApp[];
  /** Paused: bytes still needed on this disk. */
  readonly shortfall: Bytes;
}

/** Optional intents are offered only when the engine supports them (no dead buttons). */
export interface TransferIntents {
  readonly onCancel: () => void;
  readonly onPause?: () => void;
  readonly onResume?: () => void;
  readonly onIgnoreSpace?: () => void;
  readonly onOpenFiles?: () => void;
  readonly onSeeArrived?: () => void;
}
