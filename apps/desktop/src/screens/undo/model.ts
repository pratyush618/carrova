import type { Failure } from '../shared/model';

/** Undo is itself a plan you preview (the engine's `RollbackPreview`). Nothing is ever deleted. */
export interface UndoPreviewModel {
  readonly now: Date;
  /** When the move was applied: the state Undo returns to. */
  readonly pointAt: Date;
  /** What undoing would do now; `null` until the engine has answered. */
  readonly counts: UndoCounts | null;
}

/** What undoing a move would do now (the engine's preview). */
export interface UndoCounts {
  readonly restored: number;
  readonly removed: number;
  /** Changed since the move: kept in a folder, never deleted. */
  readonly movedAside: number;
  /** What undo would have to leave for the user to sort out. */
  readonly blockers: readonly Failure[];
}

export interface UndoPreviewIntents {
  readonly onKeep: () => void;
  readonly onUndo: () => void;
}

export interface UndoResultModel {
  readonly restored: number;
  readonly removed: number;
  readonly movedAside: number;
  readonly failures: readonly Failure[];
  /** Where changed files were moved instead of deleted; `null` when none were. */
  readonly movedAsideDir: string | null;
}

export interface UndoResultIntents {
  readonly onRetry: () => void;
  /** Offered where the file manager can be opened. */
  readonly onOpenMovedAside?: () => void;
  readonly onDone: () => void;
}
