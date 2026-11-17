import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { CallSetupStep } from '@/bindings/CallSetupStep';
import { useCallsSetup } from '@/flows/calls/useCallsSetup';
import { createFakePort } from '@/ipc/fake';

describe('useCallsSetup', () => {
  it.each<[CallSetupStep, CallSetupStep]>([
    ['install', 'install'],
    ['pair', 'pair'],
    ['done', 'test'],
    ['off', 'off'],
    ['no_bt', 'no_bt'],
  ])('wizard_follows_status: %s shows %s after Start', async (status, screen) => {
    const port = createFakePort({ stepMs: 0 });
    vi.spyOn(port, 'callAudioStep').mockResolvedValue(status);
    const { result } = renderHook(() => useCallsSetup(port));
    expect(result.current.step).toBe('intro');
    act(() => {
      result.current.check();
    });
    await waitFor(() => {
      expect(result.current.step).toBe(screen);
    });
  });

  it('says the install was cancelled when the password prompt was dismissed', async () => {
    const port = createFakePort({ stepMs: 0 });
    vi.spyOn(port, 'installCallAudio').mockRejectedValue({
      code: 'cancelled',
      message: 'x',
      hint: null,
    });
    const { result } = renderHook(() => useCallsSetup(port));
    act(() => {
      result.current.install();
    });
    await waitFor(() => {
      expect(result.current.note).toBe('cancelled');
    });
    expect(result.current.installed).toBe(false);
  });
});
