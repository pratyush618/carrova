import { FileChooseDialog } from '../../screens/start/FileChooseDialog';
import { HomeScreen } from '../../screens/start/HomeScreen';
import { homeAskFileManager, homeFirst, homeReturning } from '../../screens/start/fixtures';
import { noops } from '../noops';
import { states, type GalleryScreen } from '../types';

const HOME_VIEW = { kind: 'hub', page: 'home' } as const;
const homeOn = noops(
  'onSend',
  'onReceive',
  'onUseFile',
  'onUndo',
  'onHistory',
  'onErrorReports',
  'onFileManager',
);
const HOME = { first: homeFirst, 'file-manager': homeAskFileManager } as const;

export const homeBase = <HomeScreen model={homeReturning} on={homeOn} />;

export const startScreens: readonly GalleryScreen[] = [
  {
    id: 'home',
    group: 'Start',
    title: 'Home',
    view: HOME_VIEW,
    states: states(
      ['first', 'First run'],
      ['returning', 'Returning'],
      ['file-manager', 'Asks once about the file manager'],
    ),
    render: (state) => ({
      base: (
        <HomeScreen
          model={state === 'first' || state === 'file-manager' ? HOME[state] : homeReturning}
          on={homeOn}
        />
      ),
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
