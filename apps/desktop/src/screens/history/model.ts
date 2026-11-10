import type { SessionState } from '../../engine/types';
import type { Bytes } from '../shared/model';

/** What a past session did. */
export type MoveKind = 'received' | 'sent' | 'saved' | 'opened' | 'undone';

/** One past session (`carrova sessions`). */
export interface HistoryEntry {
  readonly id: string;
  readonly kind: MoveKind;
  /** The other computer's name or the file's name; `null` when not recorded. */
  readonly peer: string | null;
  readonly when: Date;
  /** Apps (and size, when recorded) the session moved. */
  readonly moved: { readonly apps: number; readonly bytes: Bytes | null } | null;
  /** A rollback point exists: Undo is possible. */
  readonly undoable: boolean;
  /** Interrupted (not finished, failed or undone): it can be picked up again. */
  readonly resumable: boolean;
}

export interface HistoryModel {
  readonly now: Date;
  readonly entries: readonly HistoryEntry[];
}

export interface HistoryIntents {
  readonly onOpen: (id: string) => void;
  readonly onReceive: () => void;
}

/** A step of a session's timeline. */
export type TimelineStep =
  | { readonly kind: 'started'; readonly at: Date }
  | { readonly kind: 'state'; readonly at: Date; readonly state: SessionState };

export interface SessionModel {
  readonly entry: HistoryEntry;
  readonly timeline: readonly TimelineStep[];
  /** Redacted by the engine: no contents, secrets or codes. */
  readonly log: readonly string[];
}

export interface SessionIntents {
  /** Forget the session and its undo point (asked to confirm first). */
  readonly onRemoveUndoPoint: () => void;
  readonly onUndo: () => void;
  readonly onResume: () => void;
  readonly onClose: () => void;
}
