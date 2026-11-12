import { AppsScreen } from '../../screens/apps/AppsScreen';
import {
  catalogAll,
  catalogCommunity,
  catalogSearch,
  catalogSelected,
  catalogSensitive,
} from '../../screens/apps/fixtures';
import type { CatalogModel } from '../../screens/apps/model';
import { HistoryScreen } from '../../screens/history/HistoryScreen';
import { historyEmpty, historyTimeline, historyUndone } from '../../screens/history/fixtures';
import { NearbyScreen } from '../../screens/nearby/NearbyScreen';
import {
  nearbyEmpty,
  nearbyList,
  nearbyMap,
  nearbyNoHint,
  nearbySelected,
} from '../../screens/nearby/fixtures';
import type { NearbyModel } from '../../screens/nearby/model';
import { ForgetDevicesDialog, KnownDevicesSheet } from '../../screens/settings/KnownDevicesSheet';
import { AboutSheet, CrashDialog, UpdateDialog } from '../../screens/settings/SettingsDialogs';
import { SECTIONS } from '../../screens/settings/sections';
import { SettingsScreen } from '../../screens/settings/SettingsScreen';
import {
  about,
  crash,
  knownDevices,
  settingsFileManagerNotes,
  settingsModel,
  settingsNoFileManager,
  update,
} from '../../screens/settings/fixtures';
import { UndoConfirmDialog } from '../../screens/undo/UndoConfirmDialog';
import { undoPreview } from '../../screens/undo/fixtures';
import { en } from '../../i18n/en';
import { noops } from '../noops';
import { homeBase } from './start';
import { states, type GalleryScreen } from '../types';

const historyOn = noops('onSelect', 'onUndo', 'onResume', 'onForget', 'onReceive');
const HISTORY = { empty: historyEmpty, undone: historyUndone } as const;
const catalogOn = noops('onFilter', 'onQuery', 'onSelect', 'onShowAll');
const APPS: Readonly<Record<string, CatalogModel>> = {
  detail: catalogSelected,
  sensitive: catalogSensitive,
  search: catalogSearch,
  community: catalogCommunity,
};
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
  'onBackground',
  'onNearby',
  'onVisibility',
  'onKnownDevices',
  'onResetIdentity',
  'onAlerts',
  'onPause',
  'onSection',
  'onBack',
);
const SETTINGS = {
  notes: settingsFileManagerNotes,
  unsupported: settingsNoFileManager,
} as const;
const settingsBase = <SettingsScreen section="general" model={settingsModel} on={settingsOn} />;
const settingsNearby = <SettingsScreen section="nearby" model={settingsModel} on={settingsOn} />;
const nearbyOn = noops(
  'onView',
  'onSelect',
  'onVisibility',
  'onSettings',
  'onSendWithCode',
  'onWake',
  'onForget',
  'onDismissWake',
);
const NEARBY: Readonly<Record<string, NearbyModel>> = {
  selected: nearbySelected,
  list: nearbyList,
  empty: nearbyEmpty,
  'no-hint': nearbyNoHint,
};

export const hubScreens: readonly GalleryScreen[] = [
  {
    id: 'nearby',
    group: 'Nearby',
    title: 'Nearby',
    view: { kind: 'hub', page: 'nearby' },
    states: states(
      ['map', 'Map'],
      ['selected', 'Device selected'],
      ['list', 'List'],
      ['empty', 'Nobody nearby'],
      ['no-hint', 'Wake without a saved address'],
    ),
    render: (s) => ({ base: <NearbyScreen model={NEARBY[s] ?? nearbyMap} on={nearbyOn} /> }),
  },
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
    states: states(
      ['list', 'List'],
      ['detail', 'App selected'],
      ['sensitive', 'Sensitive app'],
      ['search', 'Search “vs”'],
      ['community', 'Community only'],
    ),
    render: (s) => ({
      base: <AppsScreen model={APPS[s] ?? catalogAll} on={catalogOn} />,
    }),
  },
  {
    id: 'settings',
    group: 'Settings',
    title: 'Settings',
    view: { kind: 'hub', page: 'settings' },
    states: [
      ...SECTIONS.map(({ id }) => ({ id, label: en.settings.sections[id] })),
      ...states(
        ['notes', 'Sending: one file manager skipped, change failed'],
        ['unsupported', 'Sending: no file-manager entries on this system'],
      ),
    ],
    render: (s) => {
      const fixture = s === 'notes' || s === 'unsupported' ? SETTINGS[s] : settingsModel;
      const section =
        SECTIONS.find(({ id }) => id === s)?.id ??
        (fixture === settingsModel ? 'general' : 'sending');
      return { base: <SettingsScreen section={section} model={fixture} on={settingsOn} /> };
    },
  },
  {
    id: 'known-devices',
    group: 'Settings',
    title: 'Known devices (sheet)',
    view: { kind: 'hub', page: 'settings', section: 'nearby' },
    states: states(
      ['list', 'Four remembered'],
      ['forget-all', 'Forget all?'],
      ['reset', 'Reset identity?'],
    ),
    render: (s) => ({
      base: settingsNearby,
      overlays: [
        ...(s === 'reset'
          ? []
          : [
              <KnownDevicesSheet
                key="k"
                devices={knownDevices}
                now={settingsModel.now}
                on={noops('onRename', 'onForget', 'onForgetAll', 'onDone')}
              />,
            ]),
        ...(s === 'list'
          ? []
          : [
              <ForgetDevicesDialog
                key="f"
                what={s === 'reset' ? 'identity' : 'all'}
                on={noops('onCancel', 'onConfirm')}
              />,
            ]),
      ],
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
