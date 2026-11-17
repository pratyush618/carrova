import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { DesktopHost } from './host';

/** Carrova archive files. */
const ARCHIVE_FILTER = { name: 'Carrova file', extensions: ['carrova'] };

/** Label prefix of the phone-notification pop-up windows (`capabilities/note-popup.json`). */
const NOTE_POPUP = 'note-popup-';
const POPUP_SIZE = { width: 380, height: 220 };

/** The Tauri host: dialog, opener and notification plugins, loaded only inside the app. */
const tauriHost: DesktopHost = {
  pickSavePath: async (defaultPath) => {
    const { save } = await import('@tauri-apps/plugin-dialog');
    return save({ defaultPath, filters: [ARCHIVE_FILTER] });
  },
  pickOpenPath: async () => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    return open({ multiple: false, directory: false, filters: [ARCHIVE_FILTER] });
  },
  pickFiles: async (defaultPath) => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    return open({ multiple: true, directory: false, ...(defaultPath !== null && { defaultPath }) });
  },
  pickFolders: async (defaultPath) => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    return open({ multiple: true, directory: true, ...(defaultPath !== null && { defaultPath }) });
  },
  openUrl: async (url) => {
    const { openUrl } = await import('@tauri-apps/plugin-opener');
    await openUrl(url);
  },
  revealPath: async (path) => {
    const { revealItemInDir } = await import('@tauri-apps/plugin-opener');
    await revealItemInDir(path);
  },
  askNotifications: async () => {
    const { isPermissionGranted, requestPermission } =
      await import('@tauri-apps/plugin-notification');
    if (!(await isPermissionGranted())) await requestPermission();
  },
  showNotePopup: async (query) => {
    const { WebviewWindow, getAllWebviewWindows } = await import('@tauri-apps/api/webviewWindow');
    const { currentMonitor } = await import('@tauri-apps/api/window');
    // One pop-up at a time: the newest notification replaces the last.
    for (const old of await getAllWebviewWindows())
      if (old.label.startsWith(NOTE_POPUP)) await old.close();
    const monitor = await currentMonitor();
    const right = monitor ? (monitor.position.x + monitor.size.width) / monitor.scaleFactor : 0;
    const top = monitor ? monitor.position.y / monitor.scaleFactor : 0;
    new WebviewWindow(`${NOTE_POPUP}${String(Date.now())}`, {
      url: `index.html?${query}`,
      title: 'Carrova',
      width: POPUP_SIZE.width,
      height: POPUP_SIZE.height,
      ...(monitor && { x: right - POPUP_SIZE.width - 16, y: top + 16 }),
      alwaysOnTop: true,
      decorations: false,
      resizable: false,
      skipTaskbar: true,
      focus: false,
    });
  },
  notify: async (title) => {
    const { sendNotification } = await import('@tauri-apps/plugin-notification');
    sendNotification({ title });
  },
  closeWindow: async () => {
    await getCurrentWindow().close();
  },
};

/** The call window's label (`crate::call_window::CALL_WINDOW`). */
const CALL_WINDOW = 'call';

/** A plain browser (dev server, Playwright): paths are typed; links open in a new tab. */
const browserHost: DesktopHost = {
  openUrl: (url) => {
    window.open(url, '_blank', 'noopener,noreferrer');
    return Promise.resolve();
  },
};

/** Whether this build may act out the fake port (never the production bundle). */
const FAKE_ALLOWED = import.meta.env.DEV || import.meta.env.MODE !== 'production';

/**
 * With the fake port (`?fake=…`, dev and Playwright), the pickers answer at
 * once with the fake disk's paths: a folder, and a key plus a path outside home.
 */
const fakeDialogs: DesktopHost = {
  pickFolders: () => Promise.resolve(['/home/ana/projects/site']),
  pickFiles: () => Promise.resolve(['/home/ana/.ssh/id_ed25519', '/etc/hosts']),
};

/** The host for where the UI runs. */
export function detectHost(): DesktopHost {
  if (isTauri()) {
    const call = getCurrentWindow().label === CALL_WINDOW;
    return { ...tauriHost, window: call ? 'call' : 'main' };
  }
  const fake = FAKE_ALLOWED && new URLSearchParams(window.location.search).has('fake');
  return fake ? { ...browserHost, ...fakeDialogs } : browserHost;
}
