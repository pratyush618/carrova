import type { OsKind, SessionState } from '../../engine/types';
import type { Bytes } from '../shared/model';
import type { UndoCounts } from '../undo/model';

/** What a past session did. */
export type MoveKind = 'received' | 'sent' | 'saved' | 'opened' | 'undone';

/** One past session (`carrova sessions`). */
export interface HistoryEntry {
  readonly id: string;
  readonly kind: MoveKind;
  /** Where the session stopped (the engine's lifecycle state). */
  readonly state: SessionState;
  /** The other computer's name or the file's name; `null` when not recorded. */
  readonly peer: string | null;
  /** The other computer's OS; `null` when not recorded (older moves, saved files). */
  readonly peerOs: OsKind | null;
  readonly when: Date;
  /** Apps (and size, when recorded) the session moved. */
  readonly moved: { readonly apps: number; readonly bytes: Bytes | null } | null;
  /** A rollback point exists: Undo is possible. */
  readonly undoable: boolean;
  /** The undo point is still kept but its time has passed (the engine removes it next). */
  readonly undoExpired: boolean;
  /** Interrupted (not finished, failed or undone): it can be picked up again. */
  readonly resumable: boolean;
}

export interface HistoryModel {
  readonly now: Date;
  readonly entries: readonly HistoryEntry[];
  /** The move open beside the list; `null` while there are none. */
  readonly selected: SessionModel | null;
}

export interface HistoryIntents {
  readonly onSelect: (id: string) => void;
  readonly onUndo: () => void;
  readonly onResume: () => void;
  /** Forget the selected move and its undo point (asked to confirm first). */
  readonly onForget: () => void;
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
  /** What Undo would do now; `null` while it can't be undone or the engine hasn't answered. */
  readonly undoCounts: UndoCounts | null;
  /** When the undo point may be pruned; `null` = kept until the user removes it. */
  readonly undoUntil: Date | null;
}
