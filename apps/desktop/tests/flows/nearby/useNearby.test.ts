import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { NearbyPeer } from '@/bindings/NearbyPeer';
import { NEARBY_POLL_MS, useNearby } from '@/flows/nearby/useNearby';

describe('nearby looks', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('looks again every few seconds while active, and stops once not', async () => {
    let peers: NearbyPeer[] = [];
    const browse = vi.fn(() => Promise.resolve(peers));
    const { result, rerender } = renderHook(({ active }) => useNearby(browse, active), {
      initialProps: { active: true },
    });
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(result.current).toEqual([]);
    peers = [{ device_name: 'ana-desktop' }];
    await act(() => vi.advanceTimersByTimeAsync(NEARBY_POLL_MS));
    expect(result.current).toEqual([{ device_name: 'ana-desktop' }]);
    rerender({ active: false });
    const looks = browse.mock.calls.length;
    await act(() => vi.advanceTimersByTimeAsync(NEARBY_POLL_MS * 3));
    expect(browse).toHaveBeenCalledTimes(looks);
  });

  it('keeps the last list when a look fails', async () => {
    const browse = vi
      .fn<() => Promise<NearbyPeer[]>>()
      .mockResolvedValueOnce([{ device_name: 'ana-desktop' }])
      .mockRejectedValue(new Error('offline'));
    const { result } = renderHook(() => useNearby(browse, true));
    await act(() => vi.advanceTimersByTimeAsync(NEARBY_POLL_MS));
    expect(browse).toHaveBeenCalledTimes(2);
    expect(result.current).toEqual([{ device_name: 'ana-desktop' }]);
  });
});
