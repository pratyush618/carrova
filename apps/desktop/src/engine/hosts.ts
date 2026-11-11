import { isTauri } from '@tauri-apps/api/core';
import type { DesktopHost } from './host';

/** Carrova archive files. */
const ARCHIVE_FILTER = { name: 'Carrova file', extensions: ['carrova'] };

/** The Tauri host: dialog and opener plugins, loaded only inside the app. */
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
};

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
  if (isTauri()) return tauriHost;
  const fake = FAKE_ALLOWED && new URLSearchParams(window.location.search).has('fake');
  return fake ? { ...browserHost, ...fakeDialogs } : browserHost;
}
