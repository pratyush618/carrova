import { render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from '@/app/App';
import { fitWindow } from '@/app/fitWindow';
import { NotePopupRoot } from '@/app/popup/NotePopupRoot';
import type { CarrovaPort } from '@/engine/types';
import { createFakePort } from '@/ipc/fake';

const { setSize } = vi.hoisted(() => ({ setSize: vi.fn(() => Promise.resolve()) }));

vi.mock('@tauri-apps/api/core', async (actual) => ({
  ...(await actual<typeof import('@tauri-apps/api/core')>()),
  isTauri: () => true,
}));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ setSize, close: () => Promise.resolve(), label: 'main' }),
  LogicalSize: class {
    constructor(
      readonly width: number,
      readonly height: number,
    ) {}
  },
}));

/** A ResizeObserver a test reports through (jsdom has none). */
class FakeObserver {
  disconnected = false;
  constructor(private readonly callback: ResizeObserverCallback) {
    observers.push(this);
  }
  observe() {
    return undefined;
  }
  disconnect() {
    this.disconnected = true;
  }
  report() {
    this.callback([], this as unknown as ResizeObserver);
  }
}

let observers: FakeObserver[] = [];

beforeEach(() => {
  observers = [];
  vi.stubGlobal('ResizeObserver', FakeObserver);
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  setSize.mockClear();
});

/** An element `height` px tall (jsdom lays nothing out). */
function sized(height: number): HTMLElement {
  const el = document.createElement('div');
  vi.spyOn(el, 'getBoundingClientRect').mockReturnValue(DOMRect.fromRect({ height }));
  return el;
}

const reportAll = () => {
  for (const o of observers) if (!o.disconnected) o.report();
};

describe('fitWindow', () => {
  it('sizes the window to its content, keeping the width', async () => {
    const stop = fitWindow(sized(180.2));
    reportAll();
    await waitFor(() => {
      expect(setSize).toHaveBeenCalledWith({ width: window.innerWidth, height: 181 });
    });
    stop?.();
    expect(observers[0]?.disconnected).toBe(true);
  });

  it('keeps the size while nothing shows', async () => {
    fitWindow(sized(0));
    reportAll();
    await Promise.resolve();
    expect(setSize).not.toHaveBeenCalled();
  });

  it('fits the notification pop-up and the call window', async () => {
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockReturnValue(
      DOMRect.fromRect({ height: 150 }),
    );
    const port: CarrovaPort = createFakePort({ scenario: 'linked' });
    const popup = render(
      <NotePopupRoot port={port} target={{ device: 'p8', key: 'n1', name: 'Pixel 8' }} />,
    );
    await screen.findByText('Are you still coming tonight?');
    reportAll();
    await waitFor(() => {
      expect(setSize).toHaveBeenCalledTimes(1);
    });
    popup.unmount();

    const calls: CarrovaPort = createFakePort({ stepMs: 0 });
    vi.spyOn(calls, 'callNow').mockResolvedValue({
      kind: 'active',
      device: 'p8',
      since_ms: Date.now(),
      muted: false,
      audio: 'phone',
    });
    render(<App port={calls} host={{ window: 'call', closeWindow: () => Promise.resolve() }} />);
    await screen.findByRole('button', { name: 'End' });
    reportAll();
    await waitFor(() => {
      expect(setSize).toHaveBeenCalledTimes(2);
    });
  });
});
