import { NOW } from '../shared/fixtures';
import type { AboutModel, CrashModel, SettingsModel, UpdateModel } from './model';

export const settingsModel: SettingsModel = {
  now: NOW,
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
  pausedUntil: null,
  appearance: 'system',
  motion: 'system',
  autoCheckRecipes: true,
  autoCheckApp: true,
  recipes: {
    source: { kind: 'installed', version: 12 },
    verified: 13,
    community: 2,
    unverified: 0,
    check: { status: 'idle' },
  },
  errorReports: false,
  reports: [],
  update: { version: '0.2.0' },
  moveDefaults: { conflict_rule: 'replace', retention: 'until_dismissed', include_caches: false },
  startInManual: false,
  fileManager: { on: true, skipped: [], failed: false },
};

/** A desktop where one file manager was left alone, and the last change failed. */
export const settingsFileManagerNotes: SettingsModel = {
  ...settingsModel,
  fileManager: {
    on: true,
    skipped: ['Thunar: uca.xml is not valid; left unchanged'],
    failed: true,
  },
};

/** A system without file-manager entries: the row is hidden. */
export const settingsNoFileManager: SettingsModel = { ...settingsModel, fileManager: null };

export const update: UpdateModel = {
  status: 'available',
  version: '0.2.0',
  notes: 'Faster moves on busy networks, and 4 new apps.',
};

export const crash: CrashModel = {
  report: [
    'carrova 0.1.0, linux',
    'panic in apply::rename (stack trace)',
    'no file names, contents, keys or codes',
  ],
  draft: null,
};

export const about: AboutModel = {
  version: '0.1.0',
  engineVersion: '0.1.0',
  device: 'ana-laptop',
  os: 'linux',
  licence: 'Apache-2.0',
};
