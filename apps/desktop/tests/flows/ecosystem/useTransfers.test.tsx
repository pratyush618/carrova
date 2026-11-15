import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { LINK_SESSION } from '@/app/state/appState';
import { EngineStateProvider, useEngineDispatch } from '@/app/state/EngineState';
import { PortProvider } from '@/engine/port';
import type { BridgeError } from '@/engine/types';
import { useTransfers } from '@/flows/ecosystem/useTransfers';
import { createFakePort } from '@/ipc/fake';

/** A file to Pixel 8 that stopped when it left; `refusal`: what `retryFile` rejects with. */
function setup(refusal: BridgeError | null) {
  const port = createFakePort({ stepMs: 0 });
  const retryFile = vi.spyOn(port, 'retryFile');
  if (refusal === null) retryFile.mockResolvedValue();
  else retryFile.mockRejectedValue(refusal);
  const wrapper = ({ children }: { readonly children: ReactNode }) => (
    <PortProvider port={port}>
      <EngineStateProvider>{children}</EngineStateProvider>
    </PortProvider>
  );
  const hook = renderHook(
    () => ({ transfers: useTransfers(port), dispatch: useEngineDispatch() }),
    { wrapper },
  );
  act(() => {
    hook.result.current.dispatch({
      type: 'engine',
      session: LINK_SESSION,
      event: { event: 'file_failed', device: 'p8', id: 3, reason: 'device_left', retry: true },
    });
  });
  return { hook, retryFile };
}

describe('useTransfers', () => {
  it('says the device is still away when a retry cannot reach it', async () => {
    const { hook } = setup({ code: 'network', message: 'not connected', hint: null });
    act(() => {
      hook.result.current.transfers.retry();
    });
    await waitFor(() => {
      expect(hook.result.current.transfers.retryFailed).toEqual({ device: 'p8', away: true });
    });
    act(() => {
      hook.result.current.transfers.dismissRetryFailure();
    });
    expect(hook.result.current.transfers.retryFailed).toBeNull();
  });

  it('reports any other retry failure plainly', async () => {
    const { hook } = setup({ code: 'known_device', message: 'gone', hint: null });
    act(() => {
      hook.result.current.transfers.retry();
    });
    await waitFor(() => {
      expect(hook.result.current.transfers.retryFailed).toEqual({ device: 'p8', away: false });
    });
  });
});
