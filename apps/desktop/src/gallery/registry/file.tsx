import type { View } from '../../app/nav/navMachine';
import { OpenFileScreen } from '../../screens/file/OpenFileScreen';
import { SaveDoneScreen } from '../../screens/file/SaveDoneScreen';
import { SaveLockScreen } from '../../screens/file/SaveLockScreen';
import { lockStrong, lockWeak, openReady, openWrong, saved } from '../../screens/file/fixtures';
import { SendFoundScreen } from '../../screens/send/SendFoundScreen';
import { foundReady } from '../../screens/send/fixtures';
import { noops } from '../noops';
import { foundOn } from './send';
import { states, type GalleryScreen } from '../types';

const save = (step: 'what' | 'lock' | 'done'): View => ({
  kind: 'flow',
  at: { flow: 'save', step },
});

export const fileScreens: readonly GalleryScreen[] = [
  {
    id: 'save-what',
    group: 'Encrypted file',
    title: 'Save: what goes in',
    view: save('what'),
    states: [],
    render: () => ({ base: <SendFoundScreen model={foundReady} purpose="save" on={foundOn} /> }),
  },
  {
    id: 'save-lock',
    group: 'Encrypted file',
    title: 'Save: passphrase',
    view: save('lock'),
    states: states(['typing', 'Strong'], ['weak', 'Too weak']),
    render: (s) => ({
      base: (
        <SaveLockScreen
          model={s === 'weak' ? lockWeak : lockStrong}
          on={noops(
            'onPassphrase',
            'onConfirm',
            'onPath',
            'onKdf',
            'onChoosePath',
            'onBack',
            'onSave',
          )}
        />
      ),
    }),
  },
  {
    id: 'save-done',
    group: 'Encrypted file',
    title: 'Save: saved',
    view: save('done'),
    states: [],
    render: () => ({
      base: <SaveDoneScreen model={saved} on={noops('onShowInFolder', 'onDone')} />,
    }),
  },
  {
    id: 'open-file',
    group: 'Encrypted file',
    title: 'Open a file',
    view: { kind: 'flow', at: { flow: 'open', step: 'file' } },
    states: states(['ready', 'Ready'], ['wrong', 'Wrong passphrase']),
    render: (s) => ({
      base: (
        <OpenFileScreen
          model={s === 'wrong' ? openWrong : openReady}
          on={noops('onPath', 'onChooseAnother', 'onPassphrase', 'onBack', 'onOpen')}
        />
      ),
    }),
  },
];
