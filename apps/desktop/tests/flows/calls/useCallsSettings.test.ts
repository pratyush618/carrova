import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { useCallsSettings } from '@/flows/calls/useCallsSettings';
import { createFakePort } from '@/ipc/fake';

describe('useCallsSettings', () => {
  it('sends a change only once the one before it is saved', async () => {
    const port = createFakePort({ stepMs: 0 });
    let finish: () => void = () => undefined;
    const save = vi.spyOn(port, 'setPhoneSettings').mockReturnValueOnce(
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
    );
    const { result } = renderHook(() => useCallsSettings(port));
    await waitFor(() => {
      expect(result.current.phone).not.toBeNull();
    });
    act(() => {
      result.current.setPhone({ only_while_active: true });
    });
    act(() => {
      result.current.setPhone({ only_while_active: false });
    });
    await act(() => Promise.resolve());
    expect(save).toHaveBeenCalledTimes(1);
    act(() => {
      finish();
    });
    await waitFor(() => {
      expect(save).toHaveBeenCalledTimes(2);
    });
    expect(save.mock.lastCall?.[0].only_while_active).toBe(false);
  });
});
