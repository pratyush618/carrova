import { NOW } from '../shared/fixtures';
import type { AboutModel, CrashModel, SettingsModel, UpdateModel } from './model';

export const settingsModel: SettingsModel = {
  now: NOW,
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
};

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
