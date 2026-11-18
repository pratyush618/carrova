import { useEffect, useState } from 'react';
import type { CallAction } from '../../../bindings/CallAction';
import type { CallWindowView } from '../../../bindings/CallWindowView';
import { useHost } from '../../../engine/host';
import { usePort } from '../../../engine/port';
import type { CarrovaPort } from '../../../engine/types';
import { callerName } from '../../../flows/calls/callTrack';
import { useCallTrack } from '../../../flows/calls/useCallTrack';
import { useCallLog } from '../../../flows/calls/useCallLog';
import { usePhoneContacts } from '../../../flows/calls/usePhoneContacts';
import { useCallsSetup } from '../../../flows/calls/useCallsSetup';
import { callsPhone } from '../../../flows/phone/messagesPhone';
import { usePhoneSims } from '../../../flows/phone/usePhoneSims';
import { useCallWindowView } from '../../../flows/calls/useCallWindowView';
import { useLinkedName } from '../../../flows/calls/useLinkedName';
import { useRingtone } from '../../../flows/calls/useRingtone';
import { CallsSetupDialog } from '../../../screens/phone/calls-setup/CallsSetupDialog';
import { CallBar } from '../../../screens/phone/calls/CallBar';
import { CallsScreen } from '../../../screens/phone/calls/CallsScreen';
import { CallWindowScreen } from '../../../screens/phone/calls/CallWindowScreen';
import { HubLayout } from '../../../ui';
import { fitWindow } from '../../fitWindow';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';

/** The phone the shown call belongs to (`null`: none shown), so actions reach that call. */
const deviceOf = (shown: CallWindowView | null): string | null =>
  shown !== null && 'device' in shown ? shown.device : null;

/** Send `action` to the call on `device`; an outcome other than done changes nothing shown. */
function act(port: CarrovaPort, device: string | null, action: CallAction): void {
  if (device !== null) port.phoneCall(device, action).catch(() => undefined);
}

/**
 * The call window (its own always-on-top window, opened by the app when a
 * call rings here or runs): rings, shows the call, and closes itself once
 * the call is over or the phone left.
 */
export function CallWindowRoute() {
  const port = usePort();
  const { closeWindow } = useHost();
  const { track, ready, dismiss } = useCallTrack(port);
  const shown = useCallWindowView(port, track.view);
  const device = deviceOf(shown);
  const phone = useLinkedName(port, device);
  useRingtone(shown?.kind === 'ringing' && shown.ring_here);
  const sims =
    usePhoneSims(port, device ?? '', 'calls', device !== null && shown?.kind === 'missed') ?? [];
  const over = ready && track.view.kind === 'none';
  useEffect(() => {
    if (over) void closeWindow?.();
  }, [over, closeWindow]);
  const send = (action: CallAction) => {
    act(port, device, action);
  };
  return (
    <div ref={fitWindow}>
      <CallWindowScreen
        view={shown}
        who={callerName(track.caller)}
        name={track.caller?.name ?? null}
        phone={phone}
        sims={sims}
        on={{
          onAnswer: () => {
            send({ kind: 'answer' });
          },
          onDecline: () => {
            send({ kind: 'decline' });
          },
          onMute: (on) => {
            send({ kind: 'mute', on });
          },
          onTone: (digits) => {
            send({ kind: 'tones', digits });
          },
          onAudioTo: (side) => {
            send({ kind: 'audio_to', side });
          },
          onEnd: () => {
            send({ kind: 'end' });
          },
          onCallBack: (sim) => {
            if (track.caller !== null) send({ kind: 'dial', number: track.caller.number, sim });
          },
          onClose: dismiss,
        }}
      />
    </div>
  );
}

/** Home's call bar: the call ringing or running on a linked phone, or that it left. */
export function HomeCallBar() {
  const port = usePort();
  const { track } = useCallTrack(port);
  const shown = useCallWindowView(port, track.view);
  const device = deviceOf(shown);
  const phone = useLinkedName(port, device ?? track.left);
  const send = (action: CallAction) => {
    act(port, device, action);
  };
  return (
    <CallBar
      view={shown}
      who={callerName(track.caller)}
      phone={phone}
      left={track.left !== null}
      on={{
        onAnswer: () => {
          send({ kind: 'answer' });
        },
        onDecline: () => {
          send({ kind: 'decline' });
        },
        onMute: (on) => {
          send({ kind: 'mute', on });
        },
        onEnd: () => {
          send({ kind: 'end' });
        },
      }}
    />
  );
}

/** A linked phone's calls page: call from it, and its call log (off: turn calls on). */
export function CallsRoute({ id }: { readonly id: string }) {
  const port = usePort();
  const { linked } = useControllers();
  const device = linked.devices.find((d) => d.id === id);
  const on = device?.features.some((f) => f.feature === 'calls' && f.local === 'on') ?? false;
  const live = on && device?.state === 'linked';
  const { log, failed, more } = useCallLog(port, id, live);
  // Until the phone answers, a call goes from its default SIM, as with one SIM.
  const sims = usePhoneSims(port, id, 'calls', live) ?? [];
  const [query, setQuery] = useState('');
  const contacts = usePhoneContacts(port, id, query, live);
  if (device === undefined) return <HubLayout>{null}</HubLayout>;
  return (
    <CallsScreen
      model={{ phone: device.name, on, log, failed, sims, contacts }}
      on={{
        onDial: (number, sim) => {
          act(port, id, { kind: 'dial', number, sim });
        },
        onTurnOn: () => {
          linked.setFeature(id, 'calls', 'on');
        },
        onMore: more,
        onSearch: setQuery,
      }}
    />
  );
}

/** Calls: the calls page of the linked phone that shares them (empty until one does). */
export function CallsHubRoute() {
  const phone = callsPhone(useControllers().linked.devices);
  if (phone === undefined) return <HubLayout>{null}</HubLayout>;
  return <CallsRoute key={phone.id} id={phone.id} />;
}

/** Calls setup over the page that opened it. */
export function CallsSetupOverlay() {
  const nav = useNav();
  return <CallsSetupDialog setup={useCallsSetup(usePort())} onClose={nav.close} />;
}
