import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { RollbackPreview } from '@/engine/types';
import { useUndoPreview } from '@/flows/undo/useUndoPreview';
import { rollbackPreview } from '@/ipc/fixtures/rollback-preview';

describe('useUndoPreview', () => {
  it("previews the session asked for, never showing another session's counts", async () => {
    const answers: Record<string, RollbackPreview> = {
      a: rollbackPreview,
      b: { ...rollbackPreview, removed: 99 },
    };
    const peek = (id: string) => Promise.resolve(answers[id] ?? rollbackPreview);
    const { result, rerender } = renderHook(({ id }) => useUndoPreview(peek, id), {
      initialProps: { id: 'a' },
    });
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current).toBe(rollbackPreview);

    rerender({ id: 'b' });
    expect(result.current).toBeNull();
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current?.removed).toBe(99);
  });

  it('asks nothing while no session can be undone', () => {
    const peek = vi.fn(() => Promise.resolve(rollbackPreview));
    const { result } = renderHook(() => useUndoPreview(peek, null));
    expect(result.current).toBeNull();
    expect(peek).not.toHaveBeenCalled();
  });
});
