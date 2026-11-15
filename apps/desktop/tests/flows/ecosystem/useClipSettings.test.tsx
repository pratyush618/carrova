import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useClipSettings } from '@/flows/ecosystem/useClipSettings';
import { createFakePort } from '@/ipc/fake';

describe('useClipSettings', () => {
  it('says so when a change could not be saved, and shows the saved settings again', async () => {
    const port = createFakePort({ stepMs: 0 });
    vi.spyOn(port, 'setClipSettings').mockRejectedValue({
      code: 'internal',
      message: 'disk full',
      hint: null,
    });
    const hook = renderHook(() => useClipSettings(port));
    await waitFor(() => {
      expect(hook.result.current.clip?.share).toBe(true);
    });
    act(() => {
      hook.result.current.setClip({ share: false });
    });
    expect(hook.result.current.clip?.share).toBe(false);
    await waitFor(() => {
      expect(hook.result.current.failed).toBe(true);
    });
    await waitFor(() => {
      expect(hook.result.current.clip?.share).toBe(true);
    });
  });
});
