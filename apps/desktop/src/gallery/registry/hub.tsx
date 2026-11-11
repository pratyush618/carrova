import { AppsScreen } from '../../screens/apps/AppsScreen';
import { RecipeSheet } from '../../screens/apps/RecipeSheet';
import { catalogAll, catalogCommunity, recipeSsh } from '../../screens/apps/fixtures';
import { HistoryScreen } from '../../screens/history/HistoryScreen';
import { historyEmpty, historyTimeline, historyUndone } from '../../screens/history/fixtures';
import { AboutSheet, CrashDialog, UpdateDialog } from '../../screens/settings/SettingsDialogs';
import { SettingsScreen } from '../../screens/settings/SettingsScreen';
import {
  about,
  crash,
  settingsFileManagerNotes,
  settingsModel,
  settingsNoFileManager,
  update,
} from '../../screens/settings/fixtures';
import { UndoConfirmDialog } from '../../screens/undo/UndoConfirmDialog';
import { undoPreview } from '../../screens/undo/fixtures';
import { noops } from '../noops';
import { homeBase } from './start';
import { states, type GalleryScreen } from '../types';

const historyOn = noops('onSelect', 'onUndo', 'onResume', 'onForget', 'onReceive');
const HISTORY = { empty: historyEmpty, undone: historyUndone } as const;
const catalogOn = noops('onFilter', 'onQuery', 'onOpen');
const settingsOn = noops(
  'onAppearance',
  'onMotion',
  'onAutoCheckRecipes',
  'onAutoCheckApp',
  'onCheckRecipes',
  'onInstallRecipes',
  'onErrorReports',
  'onOpenReport',
  'onClearReports',
  'onUpdates',
  'onAbout',
  'onMoveDefaults',
  'onStartInManual',
  'onFileManager',
);
const SETTINGS = {
  notes: settingsFileManagerNotes,
  unsupported: settingsNoFileManager,
} as const;
const settingsBase = <SettingsScreen model={settingsModel} on={settingsOn} />;

export const hubScreens: readonly GalleryScreen[] = [
  {
    id: 'history',
    group: 'History',
    title: 'History · undo timeline',
    view: { kind: 'hub', page: 'history' },
    states: states(
      ['timeline', 'Timeline'],
      ['preview', 'Undo preview'],
      ['undone', 'Undone'],
      ['empty', 'Empty'],
    ),
    render: (s) => ({
      base: (
        <HistoryScreen
          model={s === 'empty' || s === 'undone' ? HISTORY[s] : historyTimeline}
          on={historyOn}
        />
      ),
      ...(s === 'preview' && {
        overlays: [
          <UndoConfirmDialog key="u" model={undoPreview} on={noops('onKeep', 'onUndo')} />,
        ],
      }),
    }),
  },
  {
    id: 'apps',
    group: 'Apps',
    title: 'Apps',
    view: { kind: 'hub', page: 'apps' },
    states: states(['verified', 'All'], ['community', 'Community']),
    render: (s) => ({
      base: <AppsScreen model={s === 'community' ? catalogCommunity : catalogAll} on={catalogOn} />,
    }),
  },
  {
    id: 'recipe',
    group: 'Apps',
    title: 'App detail (sheet)',
    view: { kind: 'hub', page: 'apps' },
    states: [],
    render: () => ({
      base: <AppsScreen model={catalogAll} on={catalogOn} />,
      overlays: [<RecipeSheet key="r" model={recipeSsh} on={noops('onClose')} />],
    }),
  },
  {
    id: 'settings',
    group: 'Settings',
    title: 'Settings',
    view: { kind: 'hub', page: 'settings' },
    states: states(
      ['default', 'Default'],
      ['notes', 'File manager: one skipped, change failed'],
      ['unsupported', 'No file-manager entries on this system'],
    ),
    render: (s) => ({
      base:
        s === 'notes' || s === 'unsupported' ? (
          <SettingsScreen model={SETTINGS[s]} on={settingsOn} />
        ) : (
          settingsBase
        ),
    }),
  },
  {
    id: 'update',
    group: 'Settings',
    title: 'Update ready (dialog)',
    view: { kind: 'hub', page: 'settings' },
    states: [],
    render: () => ({
      base: settingsBase,
      overlays: [
        <UpdateDialog key="u" model={update} on={noops('onLater', 'onInstall', 'onRestart')} />,
      ],
    }),
  },
  {
    id: 'crash',
    group: 'Settings',
    title: 'Error report (dialog)',
    view: { kind: 'hub', page: 'home' },
    states: [],
    render: () => ({
      base: homeBase,
      overlays: [<CrashDialog key="c" model={crash} on={noops('onDontSend', 'onSend')} />],
    }),
  },
  {
    id: 'about',
    group: 'Settings',
    title: 'About and licences (sheet)',
    view: { kind: 'hub', page: 'settings' },
    states: [],
    render: () => ({
      base: settingsBase,
      overlays: [
        <AboutSheet key="a" model={about} on={noops('onSource', 'onThirdParty', 'onClose')} />,
      ],
    }),
  },
];
