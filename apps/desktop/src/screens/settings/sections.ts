import type { IconName } from '../../ui';

export type SettingsSection =
  'general' | 'sending' | 'nearby' | 'notifications' | 'moves' | 'privacy' | 'updates' | 'about';

/** The title of every Settings row, by the id search and the sections share. */
export const ROW_TITLE = {
  theme: 'settings.device.appearance',
  motion: 'settings.device.motion',
  keepRunning: 'settings.background.keepRunning',
  startAtLogin: 'settings.background.startAtLogin',
  lean: 'settings.background.lean',
  manual: 'settings.sending.manual',
  fileManager: 'settings.sending.fileManager',
  caches: 'settings.moves.caches',
  visibility: 'settings.nearby.visibility',
  acceptKnown: 'settings.nearby.acceptKnown',
  remember: 'settings.nearby.remember',
  knownDevices: 'settings.nearby.knownDevices',
  wolHints: 'settings.nearby.wolHints',
  resetIdentity: 'settings.nearby.reset',
  system: 'settings.alerts.system',
  wakeScreen: 'settings.alerts.wakeScreen',
  lockNames: 'settings.alerts.lockNames',
  pause: 'settings.alerts.pause',
  requests: 'settings.alerts.requests',
  moves: 'settings.alerts.moves',
  nearby: 'settings.alerts.nearby',
  updates: 'settings.alerts.updates',
  conflictRule: 'settings.moves.rule',
  retention: 'settings.moves.retention',
  errorReports: 'settings.privacy.reports',
  recipes: 'settings.recipes.title',
  autoRecipes: 'settings.updates.autoRecipes',
  autoApp: 'settings.updates.autoApp',
  appUpdate: 'settings.more.updates',
  about: 'settings.more.about',
} as const;

export type SettingRowId = keyof typeof ROW_TITLE;

interface SectionDef {
  readonly id: SettingsSection;
  readonly icon: IconName;
  readonly rows: readonly SettingRowId[];
}

/** The sections the sidebar lists, in order. */
export const SECTIONS: readonly SectionDef[] = [
  {
    id: 'general',
    icon: 'settings',
    rows: ['theme', 'motion', 'keepRunning', 'startAtLogin', 'lean'],
  },
  { id: 'sending', icon: 'send', rows: ['manual', 'fileManager', 'caches'] },
  {
    id: 'nearby',
    icon: 'radar',
    // shortcut: acceptKnown hidden until Phase 3 code-less requests.
    rows: ['visibility', 'remember', 'knownDevices', 'wolHints', 'resetIdentity'],
  },
  {
    id: 'notifications',
    icon: 'bell',
    rows: ['system', 'wakeScreen', 'lockNames', 'pause', 'requests', 'moves', 'nearby', 'updates'],
  },
  { id: 'moves', icon: 'undo', rows: ['conflictRule', 'retention'] },
  { id: 'privacy', icon: 'shield', rows: ['errorReports'] },
  {
    id: 'updates',
    icon: 'refresh',
    rows: ['recipes', 'autoRecipes', 'autoApp', 'appUpdate'],
  },
  { id: 'about', icon: 'info', rows: ['about'] },
];
