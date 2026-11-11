import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { idle } from '@/flows/remote';
import { useSettings } from '@/flows/settings/useSettings';
import { createFakePort } from '@/ipc/fake';

const FAILED = { code: 'io', message: 'read-only folder', hint: null } as const;

/** Settings over the fake port, once Home would offer the file-manager entry. */
async function offered(add: 'works' | 'fails') {
  const port = createFakePort({ stepMs: 0 });
  if (add === 'fails') vi.spyOn(port, 'setFileManagerEntries').mockRejectedValue(FAILED);
  const save = vi.spyOn(port, 'saveSettings');
  const settings = renderHook(() => useSettings(port, idle));
  await waitFor(() => {
    expect(settings.result.current.offerFileManager).toBe(true);
  });
  return { settings, save };
}

describe('the one-time file-manager question', () => {
  it('file_manager_question_stays_open_when_adding_fails', async () => {
    const { settings, save } = await offered('fails');
    act(() => {
      settings.result.current.setFileManager(true);
    });
    await waitFor(() => {
      expect(settings.result.current.fileManagerFailed).toBe(true);
    });
    expect(settings.result.current.offerFileManager).toBe(true);
    expect(settings.result.current.settings?.file_manager_asked).toBe(false);
    expect(save).not.toHaveBeenCalled();
  });

  it('counts as answered once the entry is added', async () => {
    const { settings, save } = await offered('works');
    act(() => {
      settings.result.current.setFileManager(true);
    });
    await waitFor(() => {
      expect(settings.result.current.offerFileManager).toBe(false);
    });
    expect(settings.result.current.fileManagerFailed).toBe(false);
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ file_manager_asked: true }));
  });
});

describe('background settings', () => {
  const loaded = async () => {
    const port = createFakePort({ stepMs: 0 });
    const settings = renderHook(() => useSettings(port, idle));
    await waitFor(() => {
      expect(settings.result.current.settings).not.toBeNull();
    });
    return { port, settings };
  };

  it('saves them through the background command', async () => {
    const { port, settings } = await loaded();
    const background = vi.spyOn(port, 'setBackground');
    act(() => {
      settings.result.current.setBackground({ start_at_login: true });
    });
    expect(background).toHaveBeenCalledWith({
      keep_running: true,
      start_at_login: true,
      lean: true,
    });
    expect(settings.result.current.settings?.background.start_at_login).toBe(true);
  });

  it('goes back to the stored choice when the command fails', async () => {
    const { port, settings } = await loaded();
    vi.spyOn(port, 'setBackground').mockRejectedValue(FAILED);
    act(() => {
      settings.result.current.setBackground({ start_at_login: true });
    });
    await waitFor(() => {
      expect(settings.result.current.settings?.background.start_at_login).toBe(false);
    });
  });
});

describe('settings changed while the window was away', () => {
  it('reloads when the window becomes visible again', async () => {
    const port = createFakePort({ stepMs: 0 });
    const settings = renderHook(() => useSettings(port, idle));
    await waitFor(() => {
      expect(settings.result.current.settings).not.toBeNull();
    });
    const stored = settings.result.current.settings;
    if (stored === null) throw new Error('settings not loaded');
    // The tray paused alerts on the backend while this webview kept the old copy.
    const paused = { ...stored, alerts: { ...stored.alerts, paused_until: 4_000_000_000 } };
    vi.spyOn(port, 'settings').mockResolvedValue(paused);
    act(() => {
      document.dispatchEvent(new Event('visibilitychange'));
    });
    await waitFor(() => {
      expect(settings.result.current.settings?.alerts.paused_until).toBe(4_000_000_000);
    });
  });
});
