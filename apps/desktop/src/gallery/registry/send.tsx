import type { View } from '../../app/nav/navMachine';
import { CustomizeSheet } from '../../screens/send/CustomizeSheet';
import { KeyConfirmDialog } from '../../screens/send/KeyConfirmDialog';
import { SendCodeScreen } from '../../screens/send/SendCodeScreen';
import { SendDoneScreen } from '../../screens/send/SendDoneScreen';
import { SendFoundScreen } from '../../screens/send/SendFoundScreen';
import { SendLinkedScreen } from '../../screens/send/SendLinkedScreen';
import { SendLiveScreen } from '../../screens/send/SendLiveScreen';
import { UnrecognisedSheet } from '../../screens/send/UnrecognisedSheet';
import * as fx from '../../screens/send/fixtures';
import type { FoundModel, PairingModel } from '../../screens/send/model';
import { noops } from '../noops';
import { states, type GalleryScreen } from '../types';

const at = (step: 'found' | 'code' | 'live' | 'done'): View => ({
  kind: 'flow',
  at: { flow: 'send', step },
});

export const foundOn = noops(
  'onToggleApp',
  'onConfirmKey',
  'onLeaveKey',
  'onFilter',
  'onPreset',
  'onCustomize',
  'onPickFolders',
  'onSeeKnownApps',
  'onContinue',
  'onBack',
);
const sendFoundOn = { ...foundOn, ...noops('onSaveInstead') };
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
const pairingOn = noops('onBack', 'onNewCode', 'onHome');

const FOUND: Record<string, FoundModel> = {
  scanning: fx.foundScanning,
  nothing: fx.foundNothing,
};
const PAIRING: Record<string, PairingModel> = {
  waiting: fx.pairingWaiting,
  'cant-connect': fx.pairingCantConnect,
  'new-code': fx.pairingRenewed,
  locked: fx.pairingLocked,
};
const LIVE: Record<string, typeof fx.liveMoving> = {
  moving: fx.liveMoving,
  reconnecting: fx.liveReconnecting,
};

const foundReady = (
  <SendFoundScreen model={fx.foundReady} purpose="send" here={fx.HERE} on={sendFoundOn} />
);
const customize = <CustomizeSheet key="customize" model={fx.customize} on={customizeOn} />;
const [firstSecret] = fx.SECRETS;

export const sendScreens: readonly GalleryScreen[] = [
  {
    id: 'send',
    group: 'Send',
    title: 'Send · Pick what moves',
    view: at('found'),
    states: states(
      ['found', 'Found'],
      ['confirm', 'Sensitive confirm'],
      ['customize', 'Customize sheet'],
      ['scanning', 'Scanning'],
      ['nothing', 'Nothing found'],
    ),
    render: (s) => {
      const model = FOUND[s];
      if (model) {
        return {
          base: <SendFoundScreen model={model} purpose="send" here={fx.HERE} on={sendFoundOn} />,
        };
      }
      if (s === 'confirm' && firstSecret) {
        return {
          base: foundReady,
          overlays: [
            <KeyConfirmDialog key="keys" secret={firstSecret} on={noops('onBring', 'onLeave')} />,
          ],
        };
      }
      return { base: foundReady, ...(s === 'customize' && { overlays: [customize] }) };
    },
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
    id: 'pair',
    group: 'Send',
    title: 'Pair · code and handshake',
    view: at('code'),
    states: states(
      ['waiting', 'Code reveal'],
      ['linked', 'Linked (cross-OS)'],
      ['linked-same-os', 'Linked (same OS)'],
      ['cant-connect', "Can't connect"],
      ['new-code', 'Wrong code tried'],
      ['locked', '3 wrong codes'],
    ),
    render: (s) =>
      s.startsWith('linked')
        ? {
            view: at('live'),
            base: (
              <SendLinkedScreen
                model={s === 'linked' ? fx.linkedCrossOs : fx.linkedSameOs}
                on={noops('onCancel')}
              />
            ),
          }
        : {
            base: (
              <SendCodeScreen
                model={PAIRING[s] ?? fx.pairingWaiting}
                move={fx.move}
                on={pairingOn}
              />
            ),
          },
  },
  {
    id: 'send-live',
    group: 'Send',
    title: 'Sending',
    view: at('live'),
    states: states(['moving', 'Moving'], ['reconnecting', 'Reconnecting']),
    render: (s) => ({
      base: <SendLiveScreen model={LIVE[s] ?? fx.liveMoving} on={transferOn} />,
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
