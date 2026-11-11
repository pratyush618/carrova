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
  /** Not shown (nor searched) until the section has content. */
  readonly ready: boolean;
  readonly rows: readonly SettingRowId[];
}

const ALL: readonly SectionDef[] = [
  {
    id: 'general',
    icon: 'settings',
    ready: true,
    rows: ['theme', 'motion', 'keepRunning', 'startAtLogin', 'lean'],
  },
  { id: 'sending', icon: 'send', ready: true, rows: ['manual', 'fileManager', 'caches'] },
  { id: 'nearby', icon: 'radar', ready: false, rows: [] },
  {
    id: 'notifications',
    icon: 'bell',
    ready: true,
    rows: ['system', 'wakeScreen', 'lockNames', 'pause', 'requests', 'moves', 'nearby', 'updates'],
  },
  { id: 'moves', icon: 'undo', ready: true, rows: ['conflictRule', 'retention'] },
  { id: 'privacy', icon: 'shield', ready: true, rows: ['errorReports'] },
  {
    id: 'updates',
    icon: 'refresh',
    ready: true,
    rows: ['recipes', 'autoRecipes', 'autoApp', 'appUpdate'],
  },
  { id: 'about', icon: 'info', ready: true, rows: ['about'] },
];

/** The sections the sidebar lists, in order. */
export const SECTIONS: readonly Omit<SectionDef, 'ready'>[] = ALL.filter((s) => s.ready);
