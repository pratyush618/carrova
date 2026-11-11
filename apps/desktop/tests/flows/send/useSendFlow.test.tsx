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
import { raise } from '@/ipc/reject';

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

/** A promise settled by hand. */
function gate() {
  let open!: () => void;
  const opened = new Promise<void>((resolve) => {
    open = resolve;
  });
  return { opened, open };
}

const refused = { code: 'network', message: 'x', hint: null } as const;

type Flow = ReturnType<typeof setup>['flow'];

/** Starts `mode` and waits for its session. */
async function started(flow: Flow, mode: 'auto' | 'manual' = 'auto') {
  act(() => {
    flow.result.current.start(mode);
  });
  await waitFor(() => {
    expect(flow.result.current.state.selection).not.toBeNull();
  });
}

/** An Auto session with Manual opened and `SITE` picked. */
async function bothOpen(flow: Flow) {
  await started(flow);
  act(() => {
    flow.result.current.addPaths([SITE]);
  });
  await waitFor(() => {
    expect(picks(flow)).toEqual(['~/projects/site']);
  });
}

describe('useSendFlow: stale replies and failures', () => {
  it("drops an old session's late pick reply after a new start", async () => {
    const { port, flow } = setup();
    await started(flow, 'manual');
    const late = gate();
    const pick = port.pick.bind(port);
    vi.spyOn(port, 'pick').mockImplementationOnce((...args) =>
      late.opened.then(() => pick(...args)),
    );
    act(() => {
      flow.result.current.addPaths([SITE]);
    });
    await started(flow, 'manual');
    await act(async () => {
      late.open();
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    expect(picks(flow)).toEqual([]);
  });

  it('confirmSelection is false when handing the engine the visible selection fails', async () => {
    const { port, flow } = setup();
    await bothOpen(flow);
    vi.spyOn(port, 'select').mockRejectedValueOnce(refused);
    let ok = true;
    await act(async () => {
      ok = await flow.result.current.confirmSelection();
    });
    expect(ok).toBe(false);
    expect(flow.result.current.state.selectError).toMatchObject({ code: 'network' });
  });

  it("send does not run a former session's send after a new start", async () => {
    const { port, flow } = setup();
    await bothOpen(flow);
    const late = gate();
    const select = port.select.bind(port);
    vi.spyOn(port, 'select').mockImplementationOnce((...args) =>
      late.opened.then(() => select(...args)),
    );
    const runSend = vi.spyOn(port, 'runSend');
    act(() => {
      flow.result.current.send();
    });
    await started(flow);
    await act(async () => {
      late.open();
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    expect(runSend).not.toHaveBeenCalled();
  });

  it('opens Manual again after its first selection failed', async () => {
    const { port, flow } = setup();
    await started(flow);
    vi.spyOn(port, 'select').mockRejectedValueOnce(refused);
    act(() => {
      flow.result.current.switchMode('manual');
    });
    await waitFor(() => {
      expect(flow.result.current.state.selectError).not.toBeNull();
    });
    act(() => {
      flow.result.current.switchMode('auto');
    });
    act(() => {
      flow.result.current.addPaths([SITE]);
    });
    await waitFor(() => {
      expect(picks(flow)).toEqual(['~/projects/site']);
    });
  });

  it('shows a hidden app scan failure on returning to Auto, without scanning again', async () => {
    const { port, flow } = setup();
    await started(flow, 'manual');
    const late = gate();
    const scanApps = vi
      .spyOn(port, 'scanApps')
      .mockImplementationOnce(() => late.opened.then(() => raise(refused)));
    act(() => {
      flow.result.current.switchMode('auto');
    });
    act(() => {
      flow.result.current.switchMode('manual');
    });
    await act(async () => {
      late.open();
      await new Promise((resolve) => setTimeout(resolve, 10));
    });
    act(() => {
      flow.result.current.switchMode('auto');
    });
    expect(flow.result.current.state.scan).toMatchObject({ status: 'failed', error: refused });
    expect(scanApps).toHaveBeenCalledOnce();
  });
});

describe('useSendFlow: retrying a failed app scan', () => {
  it('re-scans Auto in the same session, keeping the Manual picks', async () => {
    const { port, flow } = setup();
    act(() => {
      flow.result.current.start('manual', [SITE]);
    });
    await waitFor(() => {
      expect(picks(flow)).toEqual(['~/projects/site']);
    });
    const scanApps = port.scanApps.bind(port);
    vi.spyOn(port, 'scanApps')
      .mockImplementationOnce(() => Promise.resolve().then(() => raise(refused)))
      .mockImplementation(scanApps);
    act(() => {
      flow.result.current.switchMode('auto');
    });
    await waitFor(() => {
      expect(flow.result.current.state.scan.status).toBe('failed');
    });
    act(() => {
      flow.result.current.retryScan();
    });
    await waitFor(() => {
      expect(foundModel(flow.result.current.state).status).toBe('found');
    });
    act(() => {
      flow.result.current.switchMode('manual');
    });
    expect(picks(flow)).toEqual(['~/projects/site']);
  });

  it('retries a failed Manual start as Manual, with its paths', async () => {
    const { port, flow } = setup();
    vi.spyOn(port, 'startManual').mockRejectedValueOnce(refused);
    act(() => {
      flow.result.current.start('manual', [SITE]);
    });
    await waitFor(() => {
      expect(flow.result.current.state.scan.status).toBe('failed');
    });
    act(() => {
      flow.result.current.retryScan();
    });
    await waitFor(() => {
      expect(picks(flow)).toEqual(['~/projects/site']);
    });
    expect(flow.result.current.state.mode).toBe('manual');
  });

  it('starts over when no session was opened', async () => {
    const { port, flow } = setup();
    vi.spyOn(port, 'scan').mockRejectedValueOnce(refused);
    act(() => {
      flow.result.current.start();
    });
    await waitFor(() => {
      expect(flow.result.current.state.scan.status).toBe('failed');
    });
    act(() => {
      flow.result.current.retryScan();
    });
    await waitFor(() => {
      expect(foundModel(flow.result.current.state).status).toBe('found');
    });
  });
});
