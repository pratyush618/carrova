import { NOW, YESTERDAY } from '../shared/fixtures';
import type { UndoPreviewModel, UndoResultModel } from './model';

export const undoPreview: UndoPreviewModel = {
  now: NOW,
  pointAt: YESTERDAY,
  counts: {
    restored: 3,
    removed: 412,
    movedAside: 1,
    blockers: [{ what: '~/.bashrc', reason: 'changed since apply; left in place' }],
  },
};

const result = (partial: boolean): UndoResultModel => ({
  restored: 3,
  removed: 412,
  movedAside: 1,
  failures: partial ? [{ what: '~/.bashrc', reason: 'it is open in another program' }] : [],
  movedAsideDir: '~/.local/share/carrova/moved-aside/',
});

export const undoComplete = result(false);
export const undoPartial = result(true);
