import type { View } from '../../app/nav/navMachine';
import { CustomizeSheet } from '../../screens/send/CustomizeSheet';
import { SendCodeScreen } from '../../screens/send/SendCodeScreen';
import { SendDoneScreen } from '../../screens/send/SendDoneScreen';
import { SendFoundScreen } from '../../screens/send/SendFoundScreen';
import { SendLiveScreen } from '../../screens/send/SendLiveScreen';
import { UnrecognisedSheet } from '../../screens/send/UnrecognisedSheet';
import * as fx from '../../screens/send/fixtures';
import type { FoundModel, LiveModel, PairingModel } from '../../screens/send/model';
import { noops } from '../noops';
import { states, type GalleryScreen } from '../types';

const at = (step: 'found' | 'code' | 'live' | 'done'): View => ({
  kind: 'flow',
  at: { flow: 'send', step },
});

export const foundOn = noops(
  'onToggleApp',
  'onAnswer',
  'onCustomize',
  'onPickFolders',
  'onSeeKnownApps',
  'onContinue',
  'onBack',
);
export const transferOn = noops(
  'onCancel',
  'onPause',
  'onResume',
  'onIgnoreSpace',
  'onOpenFiles',
  'onSeeArrived',
);
const customizeOn = noops(
  'onFilter',
  'onToggleApp',
  'onToggleItem',
  'onTogglePath',
  'onExpand',
  'onIncludeAnyway',
  'onUnrecognised',
  'onIncludeCaches',
  'onReset',
  'onDone',
);

const FOUND: Record<string, FoundModel> = {
  scanning: fx.foundScanning,
  ready: fx.foundReady,
  nothing: fx.foundNothing,
};
const PAIRING: Record<string, PairingModel> = {
  waiting: fx.pairingWaiting,
  'cant-connect': fx.pairingCantConnect,
  'new-code': fx.pairingRenewed,
  locked: fx.pairingLocked,
};
const LIVE: Record<string, LiveModel> = {
  reviewing: fx.liveReviewing,
  moving: fx.liveMoving,
  reconnecting: fx.liveReconnecting,
};

const foundReady = <SendFoundScreen model={fx.foundReady} purpose="send" on={foundOn} />;
const customize = <CustomizeSheet key="customize" model={fx.customize} on={customizeOn} />;

export const sendScreens: readonly GalleryScreen[] = [
  {
    id: 'send-found',
    group: 'Send',
    title: "Here's what we found",
    view: at('found'),
    states: states(['scanning', 'Scanning'], ['ready', 'Found'], ['nothing', 'Nothing found']),
    render: (s) => ({
      base: <SendFoundScreen model={FOUND[s] ?? fx.foundReady} purpose="send" on={foundOn} />,
    }),
  },
  {
    id: 'customize',
    group: 'Send',
    title: 'Customize (sheet)',
    view: at('found'),
    states: [],
    render: () => ({ base: foundReady, overlays: [customize] }),
  },
  {
    id: 'unrecognised',
    group: 'Send',
    title: "Folders Carrova doesn't recognise (sheet)",
    view: at('found'),
    states: [],
    render: () => ({
      base: foundReady,
      overlays: [
        customize,
        <UnrecognisedSheet
          key="raw"
          model={fx.unrecognised}
          on={noops('onMinSize', 'onToggle', 'onDone')}
        />,
      ],
    }),
  },
  {
    id: 'send-code',
    group: 'Send',
    title: 'Pairing code',
    view: at('code'),
    states: states(
      ['waiting', 'Waiting'],
      ['cant-connect', "Can't connect"],
      ['new-code', 'Wrong code tried'],
      ['locked', '3 wrong codes'],
    ),
    render: (s) => ({
      base: (
        <SendCodeScreen
          model={PAIRING[s] ?? fx.pairingWaiting}
          on={noops('onBack', 'onNewCode', 'onHome')}
        />
      ),
    }),
  },
  {
    id: 'send-live',
    group: 'Send',
    title: 'Connected, then sending',
    view: at('live'),
    states: states(
      ['reviewing', "They're reviewing"],
      ['moving', 'Moving'],
      ['reconnecting', 'Reconnecting'],
    ),
    render: (s) => ({
      base: <SendLiveScreen model={LIVE[s] ?? fx.liveReviewing} on={transferOn} />,
    }),
  },
  {
    id: 'send-done',
    group: 'Send',
    title: 'Sent',
    view: at('done'),
    states: [],
    render: () => ({
      base: <SendDoneScreen model={fx.delivered} on={noops('onDetails', 'onDone')} />,
    }),
  },
];
