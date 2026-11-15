import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { EngineStateProvider } from '@/app/state/EngineState';
import type { CarrovaPort } from '@/engine/types';
import { PortProvider } from '@/engine/port';
import { useLinkedDevices } from '@/flows/ecosystem/useLinkedDevices';
import { createFakePort } from '@/ipc/fake';
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
