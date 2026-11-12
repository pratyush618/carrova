import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { createElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { EngineStateProvider } from '@/app/state/EngineState';
import { PortProvider } from '@/engine/port';
import type { CarrovaPort, VisibilityView } from '@/engine/types';
import { useDevices } from '@/flows/nearby/useDevices';
import { createFakePort } from '@/ipc/fake';

/** The hook over the fake `nearby` port, once it has loaded. */
async function devices(port: CarrovaPort = createFakePort({ scenario: 'nearby', stepMs: 0 })) {
  const wrapper = ({ children }: { children: ReactNode }) =>
    createElement(PortProvider, {
      port,
      children: createElement(EngineStateProvider, { children }),
    });
  const hook = renderHook(() => useDevices(port), { wrapper });
  await waitFor(() => {
    expect(hook.result.current.visibility?.choice).toBe('known');
    expect(hook.result.current.known).toHaveLength(4);
  });
  return { port, hook };
}

describe('useDevices', () => {
  it('asks again when the window comes back (the tray or CLI may have changed things)', async () => {
    const { port, hook } = await devices();
    await port.setVisibility('no_one');
    await port.forgetDevice('all');
    act(() => {
      window.dispatchEvent(new Event('focus'));
    });
    await waitFor(() => {
      expect(hook.result.current.visibility?.choice).toBe('no_one');
      expect(hook.result.current.known).toEqual([]);
    });
  });

  it("never lets a refresh that started before the user's change undo it", async () => {
    const { port, hook } = await devices();
    let answer: (view: VisibilityView) => void = () => undefined;
    vi.spyOn(port, 'visibility').mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    );
    act(() => {
      window.dispatchEvent(new Event('focus'));
    });
    act(() => {
      hook.result.current.setVisibility('everyone');
    });
    await waitFor(() => {
      expect(hook.result.current.visibility?.choice).toBe('everyone');
    });
    // The older answer arrives last.
    await act(async () => {
      answer({ choice: 'known', until: null });
      await Promise.resolve();
    });
    expect(hook.result.current.visibility?.choice).toBe('everyone');
  });

  it('says which change the engine refused, until the next one', async () => {
    const { port, hook } = await devices();
    const refused = { code: 'known_device', message: 'no such device', hint: null } as const;
    vi.spyOn(port, 'renameDevice').mockRejectedValueOnce(refused);
    act(() => {
      hook.result.current.rename('a1', 'Ana at work');
    });
    await waitFor(() => {
      expect(hook.result.current.failed).toBe('rename');
    });
    act(() => {
      hook.result.current.forget({ device: 'a1' });
    });
    await waitFor(() => {
      expect(hook.result.current.failed).toBeNull();
      expect(hook.result.current.known).toHaveLength(3);
    });
  });

  it('drops the failure of a change that a newer one replaced', async () => {
    const { port, hook } = await devices();
    let refuse: (error: unknown) => void = () => undefined;
    vi.spyOn(port, 'setVisibility').mockImplementationOnce(
      () =>
        new Promise((_, reject) => {
          refuse = reject;
        }),
    );
    act(() => {
      hook.result.current.setVisibility('everyone');
    });
    act(() => {
      hook.result.current.setVisibility('no_one');
    });
    await waitFor(async () => {
      expect((await port.visibility()).choice).toBe('no_one');
    });
    // The older change fails last.
    act(() => {
      refuse({ code: 'internal', message: 'busy', hint: null });
    });
    await waitFor(() => {
      expect(hook.result.current.visibility?.choice).toBe('no_one');
    });
    expect(hook.result.current.failed).toBeNull();
  });
});
