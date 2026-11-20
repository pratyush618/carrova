import type { View } from '../../app/nav/navMachine';
import { DevicePage } from '../../screens/devices/DevicePage';
import { deskPc } from '../../screens/devices/fixtures';
import { deskPcRequest } from '../../screens/home/fixtures';
import { ExpiredRequestNote, MoveRequestCard } from '../../screens/home/MoveRequestCard';
import { nearbyMap } from '../../screens/nearby/fixtures';
import { NearbyScreen } from '../../screens/nearby/NearbyScreen';
import * as fx from '../../screens/send/fixtures';
import type { WaitState } from '../../screens/send/model';
import { SendLinkedScreen } from '../../screens/send/SendLinkedScreen';
import { SendToSheet } from '../../screens/send/SendToSheet';
import { SendWaitDialog } from '../../screens/send/SendWaitDialog';
import { HomeScreen } from '../../screens/start/HomeScreen';
import { homeReturning } from '../../screens/start/fixtures';
import { noops } from '../noops';
import { states, type GalleryScreen } from '../types';
import { foundReady } from './send';
import { homeOn } from './start';

const found: View = { kind: 'flow', at: { flow: 'send', step: 'found' } };
const sendToOn = noops('onSend', 'onWake', 'onUseCode', 'onCancel');
const waitOn = noops('onCancel', 'onClose', 'onRetry', 'onUseCode');
const WAIT_STATES = ['waiting', 'reviewing', 'declined', 'timeout', 'away', 'needs_code'] as const;
const isWait = (s: string): s is WaitState => WAIT_STATES.some((w) => w === s);
const nearbyOn = {
  ...noops(
    'onView',
    'onSelect',
    'onVisibility',
    'onSettings',
    'onSendWithCode',
    'onWake',
    'onForget',
    'onDismissWake',
    'onOpen',
    'onAddDevice',
    'onLinkComputer',
  ),
  ...noops('onSendSetup'),
};
const deviceOn = noops('onFeature', 'onRing', 'onStopRing', 'onSendClipboard', 'onForget');

/** Code-less moves between trusted computers (mockup `sendto`, `sendwait`, `sendreq`, `peer`, `nearby:computer`). */
export const knownSendScreens: readonly GalleryScreen[] = [
  {
    id: 'sendto',
    group: 'Move without a code',
    title: 'Send · choose computer',
    view: found,
    states: states(['choose', 'Trusted computers'], ['empty', 'None trusted yet']),
    render: (s) => ({
      base: foundReady,
      overlays: [
        <SendToSheet
          key="to"
          model={s === 'empty' ? fx.sendToEmpty : fx.sendToChoose}
          on={sendToOn}
        />,
      ],
    }),
  },
  {
    id: 'sendwait',
    group: 'Move without a code',
    title: 'Send · waiting for them',
    view: found,
    states: states(
      ['waiting', 'Request sent'],
      ['reviewing', "They're reviewing"],
      ['declined', 'Declined'],
      ['timeout', 'Timed out'],
      ['away', "Can't reach them"],
      ['needs_code', 'Needs a code now'],
    ),
    render: (s) => ({
      ...(s === 'reviewing' && {
        view: { kind: 'flow', at: { flow: 'send', step: 'live' } },
      }),
      base:
        s === 'reviewing' ? (
          <SendLinkedScreen model={fx.linkedSameOs} on={noops('onCancel')} />
        ) : (
          foundReady
        ),
      overlays: [
        <SendWaitDialog key="wait" model={fx.sendWait(isWait(s) ? s : 'waiting')} on={waitOn} />,
      ],
    }),
  },
  {
    id: 'sendreq',
    group: 'Move without a code',
    title: 'Receive · a request arrives',
    view: { kind: 'hub', page: 'home' },
    states: states(
      ['open', 'Window open'],
      ['busy', 'Another move running'],
      ['expired', 'Request expired'],
    ),
    render: (s) => ({
      base: (
        <HomeScreen
          model={homeReturning}
          on={homeOn}
          glance={
            s === 'expired' ? (
              <ExpiredRequestNote name="Desk PC" onDismiss={() => undefined} />
            ) : (
              <MoveRequestCard
                request={deskPcRequest()}
                busy={s === 'busy'}
                on={noops('onReview', 'onDecline')}
              />
            )
          }
        />
      ),
    }),
  },
  {
    id: 'nearby-computer',
    group: 'Move without a code',
    title: 'Nearby · trusted computer',
    view: { kind: 'hub', page: 'nearby' },
    states: [],
    render: () => ({
      base: <NearbyScreen model={{ ...nearbyMap, selected: 'd2' }} on={nearbyOn} />,
    }),
  },
  {
    id: 'peer',
    group: 'Move without a code',
    title: 'Device page · computer',
    view: { kind: 'device', id: 'd2' },
    states: states(['linked', 'Linked'], ['noreq', 'Send requests off'], ['away', 'Asleep']),
    render: (s) => ({
      base: (
        <DevicePage
          device={s === 'away' ? { ...deskPc, state: 'away' } : deskPc}
          move={{ sendRequests: s !== 'noreq', ...noops('onSendSetup', 'onSendRequests') }}
          on={deviceOn}
        />
      ),
    }),
  },
];
