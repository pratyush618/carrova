import { renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { SimChoice } from '@/bindings/SimChoice';
import { presetSim, usePhoneSims } from '@/flows/phone/usePhoneSims';

const sims: SimChoice[] = [
  { id: 3, slot: 1, name: 'Jio', default: false },
  { id: 7, slot: 2, name: 'Airtel', default: true },
];

describe('presetSim', () => {
  it('prefers the given SIM while offered, else the default', () => {
    expect(presetSim(sims, 3)).toBe(3);
    expect(presetSim(sims, null)).toBe(7);
    // A thread's SIM that left the phone.
    expect(presetSim(sims, 99)).toBe(7);
    expect(presetSim([], 3)).toBeNull();
  });
});

describe('usePhoneSims', () => {
  it('asks the phone while on', async () => {
    const phoneSims = vi.fn(() => Promise.resolve(sims));
    const { result } = renderHook(() => usePhoneSims({ phoneSims }, 'p8', 'calls', true));
    await waitFor(() => {
      expect(result.current).toEqual(sims);
    });
    expect(phoneSims).toHaveBeenCalledWith('p8', 'calls');
  });

  it('offers none from a phone that cannot say, or while off', async () => {
    const failing = vi.fn(() => Promise.reject(new Error('old phone')));
    const { result } = renderHook(() =>
      usePhoneSims({ phoneSims: failing }, 'p8', 'messages', true),
    );
    await waitFor(() => {
      expect(failing).toHaveBeenCalled();
    });
    expect(result.current).toEqual([]);
    const off = vi.fn(() => Promise.resolve(sims));
    renderHook(() => usePhoneSims({ phoneSims: off }, 'p8', 'messages', false));
    expect(off).not.toHaveBeenCalled();
  });
});
