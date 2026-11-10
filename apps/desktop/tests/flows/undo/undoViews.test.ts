import { describe, expect, it } from 'vitest';
import { undoPreviewModel } from '@/flows/undo/undoViews';
import { rollbackPreview } from '@/ipc/fixtures/rollback-preview';

describe('undo preview', () => {
  const at = new Date('2026-11-08T10:00:00Z');
  const now = new Date('2026-11-08T12:00:00Z');

  it('shows the counts the engine previewed, including what it cannot undo', () => {
    const model = undoPreviewModel(at, now, {
      ...rollbackPreview,
      moved_aside: 2,
      blockers: [{ target: '/home/ana/.bashrc', reason: 'changed since apply; left in place' }],
    });
    expect(model.counts).toEqual({
      restored: rollbackPreview.restored,
      removed: rollbackPreview.removed,
      movedAside: 2,
      blockers: [{ what: '/home/ana/.bashrc', reason: 'changed since apply; left in place' }],
    });
    expect(model.pointAt).toBe(at);
  });

  it('has no counts until the engine has answered', () => {
    expect(undoPreviewModel(at, now, null).counts).toBeNull();
  });
});
