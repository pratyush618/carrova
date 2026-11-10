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

/** The host for where the UI runs. */
export function detectHost(): DesktopHost {
  return isTauri() ? tauriHost : browserHost;
}
