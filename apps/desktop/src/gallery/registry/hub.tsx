import { AppsScreen } from '../../screens/apps/AppsScreen';
import { RecipeSheet } from '../../screens/apps/RecipeSheet';
import { catalogAll, catalogCommunity, recipeSsh } from '../../screens/apps/fixtures';
import { HistoryScreen } from '../../screens/history/HistoryScreen';
import { SessionSheet } from '../../screens/history/SessionSheet';
import { historyEmpty, historyList, session } from '../../screens/history/fixtures';
import { AboutSheet, CrashDialog, UpdateDialog } from '../../screens/settings/SettingsDialogs';
import { SettingsScreen } from '../../screens/settings/SettingsScreen';
import { about, crash, settingsModel, update } from '../../screens/settings/fixtures';
import { noops } from '../noops';
import { homeBase } from './start';
import { states, type GalleryScreen } from '../types';

const historyOn = noops('onOpen', 'onReceive');
const catalogOn = noops('onFilter', 'onQuery', 'onOpen');
const settingsOn = noops(
  'onAppearance',
  'onMotion',
  'onAutoCheckRecipes',
  'onAutoCheckApp',
  'onCheckRecipes',
  'onErrorReports',
  'onOpenReport',
  'onClearReports',
  'onUpdates',
  'onAbout',
  'onMoveDefaults',
);
const settingsBase = <SettingsScreen model={settingsModel} on={settingsOn} />;

export const hubScreens: readonly GalleryScreen[] = [
  {
    id: 'history',
    group: 'History',
    title: 'History',
    view: { kind: 'hub', page: 'history' },
    states: states(['list', 'With moves'], ['empty', 'Empty']),
    render: (s) => ({
      base: <HistoryScreen model={s === 'empty' ? historyEmpty : historyList} on={historyOn} />,
    }),
  },
  {
    id: 'session',
    group: 'History',
    title: 'Move details (sheet)',
    view: { kind: 'hub', page: 'history' },
    states: [],
    render: () => ({
      base: <HistoryScreen model={historyList} on={historyOn} />,
      overlays: [
        <SessionSheet
          key="s"
          model={session}
          on={noops('onRemoveUndoPoint', 'onUndo', 'onResume', 'onClose')}
        />,
      ],
    }),
  },
  {
    id: 'apps',
    group: 'Apps Carrova knows',
    title: 'Apps Carrova knows',
    view: { kind: 'hub', page: 'apps' },
    states: states(['verified', 'All'], ['community', 'Community']),
    render: (s) => ({
      base: <AppsScreen model={s === 'community' ? catalogCommunity : catalogAll} on={catalogOn} />,
    }),
  },
  {
    id: 'recipe',
    group: 'Apps Carrova knows',
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
    states: [],
    render: () => ({ base: settingsBase }),
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
