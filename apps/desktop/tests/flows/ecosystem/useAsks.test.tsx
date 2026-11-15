import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { LINK_SESSION } from '@/app/state/appState';
import { EngineStateProvider, useEngineDispatch } from '@/app/state/EngineState';
import { PortProvider } from '@/engine/port';
import type { BridgeError } from '@/engine/types';
import { useAsks } from '@/flows/ecosystem/useAsks';
import { createFakePort } from '@/ipc/fake';

/** `refusal`: what `answerAsk` rejects with (`null`: it resolves). */
function setup(refusal: BridgeError | null) {
  const port = createFakePort({ stepMs: 0 });
  const answerAsk = vi.spyOn(port, 'answerAsk');
  if (refusal === null) answerAsk.mockResolvedValue();
  else answerAsk.mockRejectedValue(refusal);
  const wrapper = ({ children }: { readonly children: ReactNode }) => (
    <PortProvider port={port}>
      <EngineStateProvider>{children}</EngineStateProvider>
    </PortProvider>
  );
  const hook = renderHook(() => ({ asks: useAsks(port), dispatch: useEngineDispatch() }), {
    wrapper,
  });
  act(() => {
    hook.result.current.dispatch({
      type: 'engine',
      session: LINK_SESSION,
      event: {
        event: 'ask_received',
        ask: 7,
        device: 'p8',
        kind: { kind: 'file', name: 'a.pdf', size: 9 },
      },
    });
  });
  return { hook, answerAsk };
}

describe('useAsks', () => {
  it('answers the waiting offer and closes it at once', async () => {
    const { hook, answerAsk } = setup(null);
    expect(hook.result.current.asks.ask?.id).toBe(7);
    act(() => {
      hook.result.current.asks.answer(true);
    });
    expect(answerAsk).toHaveBeenCalledWith(7, true);
    expect(hook.result.current.asks.ask).toBeNull();
    await act(() => Promise.resolve());
    expect(hook.result.current.asks.failed).toBeNull();
  });

  it('says nothing when the offer had already gone', async () => {
    const { hook } = setup({ code: 'known_device', message: 'gone', hint: null });
    act(() => {
      hook.result.current.asks.answer(false);
    });
    await act(() => Promise.resolve());
    expect(hook.result.current.asks.failed).toBeNull();
  });

  it('reports any other failure with the device it was for', async () => {
    const { hook } = setup({ code: 'network', message: 'down', hint: null });
    act(() => {
      hook.result.current.asks.answer(true);
    });
    await waitFor(() => {
      expect(hook.result.current.asks.failed).toBe('p8');
    });
    act(() => {
      hook.result.current.asks.dismissFailure();
    });
    expect(hook.result.current.asks.failed).toBeNull();
  });
});
