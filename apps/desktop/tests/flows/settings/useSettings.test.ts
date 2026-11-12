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
    expect(background).toHaveBeenCalledWith({ start_at_login: true });
    expect(settings.result.current.settings?.background.start_at_login).toBe(true);
  });

  it('keeps both of two quick changes, whichever lands first', async () => {
    const { port, settings } = await loaded();
    const apply = port.setBackground.bind(port);
    const sent: Parameters<typeof port.setBackground>[0][] = [];
    vi.spyOn(port, 'setBackground').mockImplementation((change) => {
      sent.push(change);
      return Promise.resolve();
    });
    act(() => {
      settings.result.current.setBackground({ start_at_login: true });
    });
    act(() => {
      settings.result.current.setBackground({ keep_running: false });
    });
    // The bridge happens to finish the second change first.
    for (const change of sent.toReversed()) await apply(change);

    expect((await port.settings()).background).toEqual({
      keep_running: false,
      start_at_login: true,
      lean: true,
    });
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

describe('saving settings', () => {
  it('keeps a pause set in the tray while the window was open', async () => {
    const port = createFakePort({ stepMs: 0 });
    const settings = renderHook(() => useSettings(port, idle));
    await waitFor(() => {
      expect(settings.result.current.settings).not.toBeNull();
    });
    // The tray pauses pop-ups behind this window's back.
    const stored = await port.settings();
    await port.saveSettings({ alerts: { ...stored.alerts, paused_until: 4_000_000_000 } });

    act(() => {
      settings.result.current.save({ theme: 'dark' });
    });

    await waitFor(async () => {
      expect((await port.settings()).theme).toBe('dark');
    });
    expect((await port.settings()).alerts.paused_until).toBe(4_000_000_000);
  });
});

describe('a pause', () => {
  it('shows as over when it ended while the settings were loading', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    try {
      const port = createFakePort({ stepMs: 0 });
      const mounted = Date.now();
      await port.saveSettings({ alerts: { paused_until: Math.floor(mounted / 1000) + 5 } });
      const settings = renderHook(() => useSettings(port, idle));
      // The pause ends before the stored settings arrive.
      vi.setSystemTime(mounted + 10_000);
      await waitFor(() => {
        expect(settings.result.current.model).not.toBeNull();
      });
      expect(settings.result.current.model?.pausedUntil).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('shows as over once its time passes, with Settings left open', async () => {
    const port = createFakePort({ stepMs: 0 });
    await port.saveSettings({ alerts: { paused_until: Math.ceil(Date.now() / 1000) + 1 } });
    const settings = renderHook(() => useSettings(port, idle));
    await waitFor(() => {
      expect(settings.result.current.model?.pausedUntil).not.toBeNull();
    });
    await waitFor(
      () => {
        expect(settings.result.current.model?.pausedUntil).toBeNull();
      },
      { timeout: 3000 },
    );
  });
});
