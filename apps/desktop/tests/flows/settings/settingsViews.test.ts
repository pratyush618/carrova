import { describe, expect, it } from 'vitest';
import { idle } from '@/flows/remote';
import type { FileManagerStatus } from '@/bindings/FileManagerStatus';
import type { Settings } from '@/bindings/Settings';
import {
  offerFileManager,
  recipeCheck,
  settingsModel,
  updateModel,
} from '@/flows/settings/settingsViews';

const SETTINGS: Settings = {
  auto_check_recipes: true,
  auto_check_app: true,
  theme: null,
  reduce_motion: null,
  move_defaults: { conflict_rule: 'replace', retention: 'until_dismissed', include_caches: false },
  start_in_manual: false,
  file_manager_asked: false,
  background: { keep_running: true, start_at_login: false, lean: true },
  alerts: {
    system: true,
    wake_screen: true,
    names_on_lock_screen: false,
    paused_until: null,
    requests: 'popup_and_bell',
    moves: 'popup_and_bell',
    nearby: 'bell_only',
    updates: 'bell_only',
  },
};

const error = { code: 'network', message: 'Offline', hint: null } as const;

describe('settings views', () => {
  it('says "try again later" when the recipe feed is unreachable', () => {
    expect(recipeCheck({ status: 'ready', value: { kind: 'retry_later' } }, idle)).toEqual({
      status: 'retry_later',
    });
    expect(
      recipeCheck({ status: 'ready', value: { kind: 'up_to_date', version: 3 } }, idle),
    ).toEqual({ status: 'up_to_date' });
  });

  it('says a rolled-back list is not offered again, not that it is up to date', () => {
    expect(recipeCheck({ status: 'ready', value: { kind: 'refused', version: 5 } }, idle)).toEqual({
      status: 'refused',
      version: 5,
    });
  });

  it('offers a newer list, then installs it only when asked', () => {
    const offered = { status: 'ready', value: { kind: 'available', version: 4 } } as const;
    expect(recipeCheck(offered, idle)).toEqual({ status: 'available', version: 4 });
    expect(recipeCheck(offered, { status: 'loading', last: null })).toEqual({
      status: 'installing',
      version: 4,
    });
    expect(
      recipeCheck(offered, { status: 'ready', value: { kind: 'updated', version: 4 } }),
    ).toEqual({ status: 'updated', version: 4 });
    expect(recipeCheck(offered, { status: 'failed', error, last: null })).toEqual({
      status: 'failed',
      error,
    });
  });

  it('follows the system theme and motion unless the user chose', () => {
    const model = settingsModel(
      {
        settings: { ...SETTINGS, auto_check_app: false, reduce_motion: true },
        fileManager: null,
        fileManagerFailed: false,
        consent: 'unset',
        reports: [{ id: '1', when: 0, kind: 'error', headline: 'Boom' }],
        recipes: null,
        check: idle,
        install: idle,
        update: {
          status: 'ready',
          value: {
            status: 'available',
            version: '0.3.0',
            current_version: '0.2.0',
            notes: null,
            date: null,
          },
        },
      },
      new Date(),
    );
    expect(model).toMatchObject({ appearance: 'system', motion: 'reduce', errorReports: false });
    expect(model.update).toEqual({ version: '0.3.0' });
    expect(model.reports).toHaveLength(1);
    expect(model.fileManager).toBeNull();
  });

  it('settings toggle reflects status: on when written, hidden when unsupported, notes kept', () => {
    const sources = (fileManager: FileManagerStatus | null, fileManagerFailed = false) =>
      settingsModel(
        {
          settings: { ...SETTINGS, start_in_manual: true },
          fileManager,
          fileManagerFailed,
          consent: 'denied',
          reports: [],
          recipes: null,
          check: idle,
          install: idle,
          update: idle,
        },
        new Date(),
      );
    const skipped = ['Thunar: uca.xml is not valid; left unchanged'];
    expect(sources({ supported: true, installed: true, skipped })).toMatchObject({
      startInManual: true,
      fileManager: { on: true, skipped, failed: false },
    });
    expect(sources({ supported: true, installed: false, skipped: [] }, true).fileManager).toEqual({
      on: false,
      skipped: [],
      failed: true,
    });
    expect(sources({ supported: false, installed: false, skipped: [] }).fileManager).toBeNull();
  });

  it('home asks once about the file manager: only where supported, not yet written or answered', () => {
    const status = { supported: true, installed: false, skipped: [] };
    expect(offerFileManager(SETTINGS, status)).toBe(true);
    expect(offerFileManager({ ...SETTINGS, file_manager_asked: true }, status)).toBe(false);
    expect(offerFileManager(SETTINGS, { ...status, installed: true })).toBe(false);
    expect(offerFileManager(SETTINGS, { ...status, supported: false })).toBe(false);
    expect(offerFileManager(SETTINGS, null)).toBe(false);
    expect(offerFileManager(null, status)).toBe(false);
  });

  it('walks the update dialog from offer to installed, and shows failures', () => {
    const offer = {
      status: 'ready',
      value: {
        status: 'available',
        version: '0.3.0',
        current_version: '0.2.0',
        notes: 'New',
        date: null,
      },
    } as const;
    expect(updateModel(offer, idle, null)).toEqual({
      status: 'available',
      version: '0.3.0',
      notes: 'New',
    });
    expect(updateModel(offer, { status: 'loading', last: null }, '0.3.0').status).toBe(
      'installing',
    );
    expect(updateModel(offer, { status: 'ready', value: undefined }, '0.3.0').status).toBe(
      'installed',
    );
    expect(updateModel({ status: 'failed', error, last: null }, idle, null)).toEqual({
      status: 'failed',
      error,
    });
  });
});
