import { NOW, YESTERDAY } from '../shared/fixtures';
import type { AboutModel, CrashModel, KnownDeviceLine, SettingsModel, UpdateModel } from './model';

export const settingsModel: SettingsModel = {
  now: NOW,
  background: { keep_running: true, start_at_login: false, lean: true },
  minimisesOnClose: false,
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
  nearby: { remember: true, accept_known: true, wol_hints: false },
  visibility: 'known',
  knownDevices: 4,
  devicesFailed: null,
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

/** The mockup's four remembered devices. */
export const knownDevices: readonly KnownDeviceLine[] = [
  { id: 'a1', name: "Ana's laptop", os: 'linux', fingerprint: '7F3A·19C2', lastMove: YESTERDAY },
  { id: 'd2', name: 'Desk PC', os: 'windows', fingerprint: 'B04E·88D1', lastMove: NOW },
  { id: 'o3', name: 'Old ThinkPad', os: 'linux', fingerprint: '2C9F·4A07', lastMove: null },
  { id: 'w4', name: 'Work laptop', os: 'windows', fingerprint: 'E51B·03C6', lastMove: YESTERDAY },
];
