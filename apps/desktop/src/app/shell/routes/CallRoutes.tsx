import { useEffect } from 'react';
import type { CallAction } from '../../../bindings/CallAction';
import type { CallView } from '../../../bindings/CallView';
import { useHost } from '../../../engine/host';
import { usePort } from '../../../engine/port';
import type { CarrovaPort } from '../../../engine/types';
import { callerName } from '../../../flows/calls/callTrack';
import { useCallTrack } from '../../../flows/calls/useCallTrack';
import { useCallLog } from '../../../flows/calls/useCallLog';
import { useCallsSetup } from '../../../flows/calls/useCallsSetup';
import { usePhoneSims } from '../../../flows/phone/usePhoneSims';
import { useCallWindowView } from '../../../flows/calls/useCallWindowView';
import { useLinkedName } from '../../../flows/calls/useLinkedName';
import { useRingtone } from '../../../flows/calls/useRingtone';
import { CallsSetupDialog } from '../../../screens/phone/calls-setup/CallsSetupDialog';
import { CallBar } from '../../../screens/phone/calls/CallBar';
import { CallsScreen } from '../../../screens/phone/calls/CallsScreen';
import { CallWindowScreen } from '../../../screens/phone/calls/CallWindowScreen';
import { HubLayout } from '../../../ui';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';

/** The phone a call view belongs to (`null`: no call). */
const deviceOf = (view: CallView): string | null => ('device' in view ? view.device : null);

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
  const device = deviceOf(track.view);
  const phone = useLinkedName(port, device);
  useRingtone(shown?.kind === 'ringing' && shown.ring_here);
  const over = ready && track.view.kind === 'none';
  useEffect(() => {
    if (over) void closeWindow?.();
  }, [over, closeWindow]);
  const send = (action: CallAction) => {
    act(port, device, action);
  };
  return (
    <CallWindowScreen
      view={shown}
      who={callerName(track.caller)}
      phone={phone}
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
        onCallBack: () => {
          // The phone's default SIM: this window offers no choice.
          if (track.caller !== null) send({ kind: 'dial', number: track.caller.number, sim: null });
        },
        onClose: dismiss,
      }}
    />
  );
}

/** Home's call bar: the call ringing or running on a linked phone, or that it left. */
export function HomeCallBar() {
  const port = usePort();
  const { track } = useCallTrack(port);
  const shown = useCallWindowView(port, track.view);
  const device = deviceOf(track.view);
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
  const sims = usePhoneSims(port, id, 'calls', live);
  if (device === undefined) return <HubLayout>{null}</HubLayout>;
  return (
    <CallsScreen
      model={{ phone: device.name, on, log, failed, sims }}
      on={{
        onDial: (number, sim) => {
          act(port, id, { kind: 'dial', number, sim });
        },
        onTurnOn: () => {
          linked.setFeature(id, 'calls', 'on');
        },
        onMore: more,
      }}
    />
  );
}

/** Calls setup over the page that opened it. */
export function CallsSetupOverlay() {
  const nav = useNav();
  return <CallsSetupDialog setup={useCallsSetup(usePort())} onClose={nav.close} />;
}
