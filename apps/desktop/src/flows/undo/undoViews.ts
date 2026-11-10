import type { RollbackPreview } from '../../engine/types';
import type { UndoCounts, UndoPreviewModel } from '../../screens/undo/model';

/** The engine's undo preview as counts (`null` until it has answered). */
export function undoCounts(preview: RollbackPreview | null): UndoCounts | null {
  return preview === null
    ? null
    : {
        restored: preview.restored,
        removed: preview.removed,
        movedAside: preview.moved_aside,
        blockers: preview.blockers.map((b) => ({ what: b.target, reason: b.reason })),
      };
}

/**
 * The undo question for a move applied at `pointAt`: the engine's preview of
 * what undoing it would do now (`null` until it has answered).
 */
export function undoPreviewModel(
  pointAt: Date,
  now: Date,
  preview: RollbackPreview | null,
): UndoPreviewModel {
  return { now, pointAt, counts: undoCounts(preview) };
}
