import { describe, expect, it, vi } from 'vitest';

/** The fake Tauri windows: a new one lists only once created (a tick later). */
const windows = vi.hoisted(() => {
  const open = new Set<{ label: string; url: string; close: () => Promise<void> }>();
  class FakeWebviewWindow {
    private created: Promise<void>;
    constructor(
      readonly label: string,
      { url }: { url: string },
    ) {
      const win = { label, url, close: () => Promise.resolve(void open.delete(win)) };
      this.created = new Promise((resolve) => {
        setTimeout(() => {
          open.add(win);
          resolve();
        }, 5);
      });
    }
    once(event: string, handler: () => void) {
      if (event === 'tauri://created') void this.created.then(handler);
      return Promise.resolve(() => undefined);
    }
  }
  return { open, FakeWebviewWindow };
});

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }));
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  WebviewWindow: windows.FakeWebviewWindow,
  getAllWebviewWindows: () => Promise.resolve([...windows.open]),
}));
vi.mock('@tauri-apps/api/window', () => ({
  currentMonitor: () => Promise.resolve(null),
  getCurrentWindow: () => ({ label: 'main' }),
}));

const { detectHost } = await import('@/engine/hosts');

describe('the Tauri host', () => {
  it('keeps one note pop-up when notes arrive together: the newest', async () => {
    const host = detectHost();
    await Promise.all([host.showNotePopup?.('key=a'), host.showNotePopup?.('key=b')]);
    // Let any window still being created land.
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect([...windows.open].map((w) => w.url)).toEqual(['index.html?key=b']);
  });
});
