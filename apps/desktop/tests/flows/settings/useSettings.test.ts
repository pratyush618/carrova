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
