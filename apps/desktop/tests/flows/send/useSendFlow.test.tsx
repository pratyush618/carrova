import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi, type MockInstance } from 'vitest';
import { EngineStateProvider } from '@/app/state/EngineState';
import type { DesktopHost } from '@/engine/host';
import { PortProvider } from '@/engine/port';
import type { CarrovaPort } from '@/engine/types';
import { foundModel } from '@/flows/send/sendProgressViews';
import { useSendFlow } from '@/flows/send/useSendFlow';
import { createFakePort } from '@/ipc/fake';

const SITE = '/home/ana/projects/site';

function setup(host: DesktopHost = {}) {
  const port = createFakePort({ stepMs: 0 });
  const wrapper = ({ children }: { readonly children: ReactNode }) => (
    <PortProvider port={port}>
      <EngineStateProvider>{children}</EngineStateProvider>
    </PortProvider>
  );
  const flow = renderHook(() => useSendFlow(port, false, host), { wrapper });
  return { port, flow };
}

/** Picks listed on the visible Manual screen. */
const picks = (flow: ReturnType<typeof setup>['flow']): string[] => {
  const model = foundModel(flow.result.current.state);
  return model.status === 'manual' ? model.picks.map((p) => p.path) : [];
};

/** Keys of the selection the engine was last handed. */
const lastSelected = (spy: MockInstance<CarrovaPort['select']>): string[] =>
  Object.keys(spy.mock.lastCall?.[1].items ?? {});

describe('useSendFlow: Manual mode', () => {
  it('next acts on the visible mode only', async () => {
    const { port, flow } = setup();
    act(() => {
      flow.result.current.start();
    });
    await waitFor(() => {
      expect(flow.result.current.state.selection).not.toBeNull();
    });
    act(() => {
      flow.result.current.switchMode('manual');
    });
    act(() => {
      flow.result.current.addPaths([SITE]);
    });
    await waitFor(() => {
      expect(picks(flow)).toEqual(['~/projects/site']);
    });

    const select = vi.spyOn(port, 'select');
    act(() => {
      flow.result.current.switchMode('auto');
    });
    let ok = false;
    await act(async () => {
      ok = await flow.result.current.confirmSelection();
    });
    expect(ok).toBe(true);
    const auto = lastSelected(select);
    expect(auto).toContain('git/config');
    expect(auto.some((k) => k.startsWith('raw:home/projects'))).toBe(false);

    act(() => {
      flow.result.current.switchMode('manual');
    });
    await act(async () => {
      ok = await flow.result.current.confirmSelection();
    });
    expect(ok).toBe(true);
    expect(lastSelected(select)).toEqual(['raw:home/projects/site']);
  });

  it('a manual start skips the scan and scans apps once, on the first switch to Auto', async () => {
    const { port, flow } = setup();
    const scan = vi.spyOn(port, 'scan');
    const scanApps = vi.spyOn(port, 'scanApps');
    act(() => {
      flow.result.current.start('manual', [SITE]);
    });
    await waitFor(() => {
      expect(picks(flow)).toEqual(['~/projects/site']);
    });
    expect(scan).not.toHaveBeenCalled();
    expect(scanApps).not.toHaveBeenCalled();

    act(() => {
      flow.result.current.switchMode('auto');
    });
    await waitFor(() => {
      expect(foundModel(flow.result.current.state).status).toBe('found');
    });
    act(() => {
      flow.result.current.switchMode('manual');
    });
    act(() => {
      flow.result.current.switchMode('auto');
    });
    expect(scanApps).toHaveBeenCalledOnce();
    const items = Object.keys(flow.result.current.state.selection?.items ?? {});
    expect(items.some((k) => k.startsWith('raw:home/projects'))).toBe(false);
    act(() => {
      flow.result.current.switchMode('manual');
    });
    expect(picks(flow)).toEqual(['~/projects/site']);
  });

  it('opens the file picker in home and removes a pick on request', async () => {
    const pickFiles = vi.fn(() => Promise.resolve(['/home/ana/notes.txt']));
    const { flow } = setup({ pickFiles });
    act(() => {
      flow.result.current.start('manual');
    });
    await waitFor(() => {
      expect(flow.result.current.state.session).not.toBeNull();
    });
    act(() => {
      flow.result.current.addFiles?.();
    });
    await waitFor(() => {
      expect(picks(flow)).toEqual(['~/notes.txt']);
    });
    expect(pickFiles).toHaveBeenCalledWith('/home/ana');
    expect(flow.result.current.addFolders).toBeNull();

    act(() => {
      flow.result.current.removePick('raw:home/notes.txt');
    });
    await waitFor(() => {
      expect(picks(flow)).toEqual([]);
    });
  });
  it('a resumed send shows Auto, and the Manual switch works again', async () => {
    const { flow } = setup();
    act(() => {
      flow.result.current.start('manual', [SITE]);
    });
    await waitFor(() => {
      expect(picks(flow)).toEqual(['~/projects/site']);
    });
    const id = flow.result.current.state.session;
    if (id === null) throw new Error('no session');
    act(() => {
      flow.result.current.resume(id);
    });
    expect(flow.result.current.state.mode).toBe('auto');
    act(() => {
      flow.result.current.switchMode('manual');
    });
    expect(flow.result.current.state.mode).toBe('manual');
  });
});
