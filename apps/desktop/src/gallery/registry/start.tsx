import { FileChooseDialog } from '../../screens/start/FileChooseDialog';
import { HomeScreen } from '../../screens/start/HomeScreen';
import { WelcomeScreen } from '../../screens/start/WelcomeScreen';
import { homeFirst, homeReturning, welcome } from '../../screens/start/fixtures';
import { noops } from '../noops';
import { states, type GalleryScreen } from '../types';

const HOME_VIEW = { kind: 'hub', page: 'home' } as const;
const homeOn = noops('onSend', 'onReceive', 'onUseFile', 'onUndoLast');

export const homeBase = <HomeScreen model={homeReturning} on={homeOn} />;

export const startScreens: readonly GalleryScreen[] = [
  {
    id: 'welcome',
    group: 'Start',
    title: 'Welcome (first run)',
    view: { kind: 'welcome' },
    states: [],
    render: () => ({
      base: <WelcomeScreen model={welcome} on={noops('onErrorReports', 'onStart')} />,
    }),
  },
  {
    id: 'home',
    group: 'Start',
    title: 'Home',
    view: HOME_VIEW,
    states: states(['returning', 'Returning'], ['first', 'No moves yet']),
    render: (state) => ({
      base: <HomeScreen model={state === 'first' ? homeFirst : homeReturning} on={homeOn} />,
    }),
  },
  {
    id: 'file-choose',
    group: 'Start',
    title: 'Use a file instead (dialog)',
    view: HOME_VIEW,
    states: [],
    render: () => ({
      base: homeBase,
      overlays: [<FileChooseDialog key="d" on={noops('onSave', 'onOpen', 'onCancel')} />],
    }),
  },
];
