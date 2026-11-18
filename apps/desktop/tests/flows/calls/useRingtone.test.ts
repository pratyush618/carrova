import { renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { useRingtone } from '@/flows/calls/useRingtone';

describe('useRingtone', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('does not start ringing once stopped before it played', async () => {
    const play = vi.spyOn(window.HTMLMediaElement.prototype, 'play').mockResolvedValue();
    vi.spyOn(window.HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
    const { unmount } = renderHook(() => {
      useRingtone(true);
    });
    unmount();
    await Promise.resolve();
    await Promise.resolve();
    expect(play).not.toHaveBeenCalled();
  });
});
