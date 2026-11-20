import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { EngineStateProvider } from '@/app/state/EngineState';
import type { BridgeEvent } from '@/bindings/BridgeEvent';
import type { CarrovaPort } from '@/engine/types';
import { PortProvider } from '@/engine/port';
import { useLinkedDevices } from '@/flows/ecosystem/useLinkedDevices';
import { createFakePort } from '@/ipc/fake';
import { raise } from '@/ipc/reject';
import { pixel } from '@/screens/devices/fixtures';

const CANCELLED = { code: 'cancelled', message: 'cancelled', hint: null } as const;

function setup(port: CarrovaPort = createFakePort({ stepMs: 0 })) {
  const wrapper = ({ children }: { readonly children: ReactNode }) => (
    <PortProvider port={port}>
      <EngineStateProvider>{children}</EngineStateProvider>
    </PortProvider>
  );
  return { port, hook: renderHook(() => useLinkedDevices(port), { wrapper }) };
}

/** Lets queued promise callbacks run. */
const settle = () => act(() => new Promise((resolve) => setTimeout(resolve, 0)));

describe('useLinkedDevices pairing', () => {
  it('starts a new code only once the cancelled pairing has ended (the engine pairs one at a time)', async () => {
    const { port, hook } = setup();
    let endFirst: (reason: unknown) => void = () => undefined;
    const invite = vi
      .spyOn(port, 'pairInvite')
      .mockImplementationOnce(
        () =>
          new Promise((_, reject) => {
            endFirst = reject;
          }),
      )
      .mockImplementationOnce(() => Promise.resolve());
    const cancel = vi.spyOn(port, 'cancelConnect');

    const first = hook.result.current.pairDevice();
    first.done.catch(() => undefined);
    await settle();
    expect(invite).toHaveBeenCalledOnce();
    first.cancel();
    expect(cancel).toHaveBeenCalledOnce();

    const second = hook.result.current.pairDevice();
    await settle();
    expect(invite).toHaveBeenCalledOnce();

    endFirst(CANCELLED);
    await expect(first.done).rejects.toEqual(CANCELLED);
    await expect(second.done).resolves.toBeNull();
    expect(invite).toHaveBeenCalledTimes(2);
  });

  it('never starts a pairing cancelled before its turn (StrictMode mounts twice)', async () => {
    const { port, hook } = setup();
    const invite = vi.spyOn(port, 'pairInvite');
    const cancel = vi.spyOn(port, 'cancelConnect');
    const first = hook.result.current.pairDevice();
    first.cancel();
    await expect(first.done).rejects.toEqual(CANCELLED);
    expect(invite).not.toHaveBeenCalled();
    expect(cancel).not.toHaveBeenCalled();
  });

  it('names a known device linked again (its id is not new)', async () => {
    const port = createFakePort({ stepMs: 0, scenario: 'linked' });
    const { hook } = setup(port);
    const away = { ...pixel, state: 'away' } as const;
    vi.spyOn(port, 'linkedDevices')
      .mockResolvedValueOnce([away])
      .mockResolvedValue([{ ...away, state: 'linked' }]);
    await act(() => hook.result.current.reload());
    vi.spyOn(port, 'pairInvite').mockResolvedValue();
    await expect(hook.result.current.pairDevice().done).resolves.toMatchObject({ id: 'p8' });
  });
});

describe('useLinkedDevices linking a computer', () => {
  it('links from a pasted pairing link, and from a typed code', async () => {
    const { port, hook } = setup();
    const invite = vi.spyOn(port, 'linkInvite').mockResolvedValue();
    const code = vi.spyOn(port, 'linkComputer').mockResolvedValue();
    const link = 'carrova://pair?v=1&t=4-maple-orbit&k=KEY';

    await hook.result.current.linkComputer(` ${link} `).done;
    await hook.result.current.linkComputer('4-maple-orbit').done;

    expect(invite).toHaveBeenCalledExactlyOnceWith(link);
    expect(code).toHaveBeenCalledExactlyOnceWith('4-maple-orbit');
  });
});

describe('useLinkedDevices features', () => {
  it('shows a refused change as failed and loads the list again', async () => {
    const port = createFakePort({ stepMs: 0, scenario: 'linked' });
    const { hook } = setup(port);
    await waitFor(() => {
      expect(hook.result.current.devices).toHaveLength(2);
    });
    vi.spyOn(port, 'setFeature').mockRejectedValue({
      code: 'known_device',
      message: 'm',
      hint: null,
    });
    const load = vi.spyOn(port, 'linkedDevices');
    act(() => {
      hook.result.current.setFeature('p8', 'files', 'off');
    });
    await waitFor(() => {
      expect(hook.result.current.failed).toBe(true);
    });
    expect(load).toHaveBeenCalled();
    // The reload put the engine's value back.
    expect(
      hook.result.current.devices
        .find((d) => d.id === 'p8')
        ?.features.find((f) => f.feature === 'files')?.local,
    ).toBe('ask');
  });
});

describe('useLinkedDevices ring', () => {
  it('shows a ring in progress until it is stopped', async () => {
    const port = createFakePort({ stepMs: 0, scenario: 'linked' });
    const stop = vi.spyOn(port, 'stopRing');
    const { hook } = setup(port);
    await waitFor(() => {
      expect(hook.result.current.devices).toHaveLength(2);
    });
    await act(() => hook.result.current.ring('p8'));
    expect(hook.result.current.ringing.has('p8')).toBe(true);
    await act(() => hook.result.current.stopRing('p8'));
    expect(stop).toHaveBeenCalledWith('p8');
    expect(hook.result.current.ringing.has('p8')).toBe(false);
  });

  it('ends a ring by itself after 30 seconds, and not before', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const { hook } = setup(createFakePort({ stepMs: 0, scenario: 'linked' }));
      await waitFor(() => {
        expect(hook.result.current.devices).toHaveLength(2);
      });
      await act(() => hook.result.current.ring('p8'));
      act(() => {
        vi.advanceTimersByTime(29_000);
      });
      expect(hook.result.current.ringing.has('p8')).toBe(true);
      act(() => {
        vi.advanceTimersByTime(1_000);
      });
      expect(hook.result.current.ringing.has('p8')).toBe(false);
    } finally {
      vi.useRealTimers();
    }
  });

  it('shows no ring when the ring was refused', async () => {
    const port = createFakePort({ stepMs: 0, scenario: 'linked' });
    vi.spyOn(port, 'ring').mockRejectedValue({ code: 'known_device', message: 'm', hint: null });
    const { hook } = setup(port);
    await act(() => hook.result.current.ring('p8').catch(() => undefined));
    expect(hook.result.current.ringing.has('p8')).toBe(false);
  });
});

describe('useLinkedDevices sendFiles', () => {
  const refused = { code: 'known_device', message: 'declined', hint: null } as const;

  /** A port whose file send to Pixel 8 fails, reporting `file_failed` before or after the reply if `reported`. */
  function failing(reported: false | 'before' | 'after') {
    const port = createFakePort({ stepMs: 0, scenario: 'linked' });
    const listeners: ((event: BridgeEvent) => void)[] = [];
    const subscribe = port.subscribe.bind(port);
    vi.spyOn(port, 'subscribe').mockImplementation((listener) => {
      listeners.push(listener);
      return subscribe(listener);
    });
    const report = () => {
      for (const listener of listeners)
        listener({
          kind: 'engine',
          session: 'link',
          event: { event: 'file_failed', device: 'p8', id: 4, reason: 'refused', retry: false },
        });
    };
    vi.spyOn(port, 'sendFiles').mockImplementation(() => {
      if (reported === 'before') report();
      if (reported === 'after') setTimeout(report, 10);
      return Promise.resolve().then(() => raise(refused));
    });
    return port;
  }

  it('leaves a failure the engine reported as file_failed to its own note', async () => {
    const { hook } = setup(failing('before'));
    await expect(hook.result.current.sendFiles('p8', ['/a.txt'])).resolves.toBeUndefined();
  });

  it('leaves it to the note when file_failed lands just after the reply', async () => {
    const { hook } = setup(failing('after'));
    await expect(hook.result.current.sendFiles('p8', ['/a.txt'])).resolves.toBeUndefined();
  });

  it('rejects a send that failed before any file was offered', async () => {
    const { hook } = setup(failing(false));
    await expect(hook.result.current.sendFiles('p8', ['/a.txt'])).rejects.toEqual(refused);
  });
});
