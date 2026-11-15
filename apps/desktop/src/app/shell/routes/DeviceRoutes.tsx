import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useHost } from '../../../engine/host';
import type { LinkedDeviceView } from '../../../engine/types';
import { usePort } from '../../../engine/port';
import { toBridgeError } from '../../../engine/errors';
import { useClipHistory } from '../../../flows/ecosystem/useClipHistory';
import type { Pairing } from '../../../flows/ecosystem/useLinkedDevices';
import {
  addDeviceModel,
  attemptInvite,
  attemptPeer,
  settle,
  type PairEnd,
} from '../../../flows/ecosystem/pairing';
import { ClipHistorySheet } from '../../../screens/clipboard/ClipHistorySheet';
import { AddDeviceDialog } from '../../../screens/devices/AddDeviceDialog';
import { DevicePage } from '../../../screens/devices/DevicePage';
import { deviceKind } from '../../../screens/devices/model';
import { ForgetDialog } from '../../../screens/devices/ForgetDialog';
import {
  LinkComputerDialog,
  type LinkComputerState,
} from '../../../screens/devices/LinkComputerDialog';
import { useSecondsLeft } from '../../../screens/send/countdown';
import { HubLayout, Toast } from '../../../ui';
import { useControllers } from '../../controllers/Controllers';
import { useInvite } from '../../state/EngineState';
import { useNav } from '../nav';

/** What the device page last said about an action (sent, or the device didn't answer). */
type ActionNote = 'clipSent' | 'actionFailed';

/** One linked device's page; empty until the list has it (and after it is forgotten). */
export function DeviceRoute({ id }: { readonly id: string }) {
  const { t } = useTranslation();
  const { linked } = useControllers();
  const host = useHost();
  const nav = useNav();
  const [note, setNote] = useState<ActionNote | null>(null);
  const device = linked.devices.find((d) => d.id === id);
  if (device === undefined) return <HubLayout>{null}</HubLayout>;
  const failed = () => {
    setNote('actionFailed');
  };
  const { pickFiles } = host;
  return (
    <>
      <DevicePage
        device={device}
        failed={linked.failed}
        ringing={linked.ringing.has(id)}
        on={{
          onFeature: (feature, state) => {
            linked.setFeature(id, feature, state);
          },
          onRing: () => {
            linked.ring(id).catch(failed);
          },
          onStopRing: () => {
            linked.stopRing(id).catch(failed);
          },
          onSendClipboard: () => {
            linked.sendClipboard(id).then(() => {
              setNote('clipSent');
            }, failed);
          },
          ...(pickFiles && {
            onSendFiles: () => {
              void pickFiles(null).then((paths) => {
                if (paths !== null && paths.length > 0) linked.sendFiles(id, paths).catch(failed);
              });
            },
          }),
          onForget: () => {
            nav.open({ kind: 'forget-device', id });
          },
        }}
      />
      {note !== null && (
        <Toast
          icon={note === 'clipSent' ? 'check' : 'warn'}
          onDismiss={() => {
            setNote(null);
          }}
        >
          {t(`ecosystem.device.${note}`, { name: device.name })}
        </Toast>
      )}
    </>
  );
}

/**
 * Add a device: one invite per attempt; closing the dialog or a new code
 * cancels the one waiting. The code leaves memory once the dialog closes or
 * the code runs out (a device found keeps it until answered: it is used up).
 */
export function AddDeviceOverlay() {
  const { linked } = useControllers();
  const port = usePort();
  const nav = useNav();
  const { invite, found, clear } = useInvite();
  const [attempt, setAttempt] = useState(() => Date.now());
  const [end, setEnd] = useState<PairEnd | null>(null);
  // The user answered the device found; the engine finishes the pairing.
  const [answered, setAnswered] = useState(false);
  const { pairDevice } = linked;
  useEffect(() => {
    const pairing = pairDevice();
    let live = true;
    const ended = (next: PairEnd) => {
      if (live) setEnd((now) => settle(now, next));
    };
    pairing.done.then(
      (device) => {
        ended({ state: 'linked', device });
      },
      (reason: unknown) => {
        if (toBridgeError(reason).code !== 'cancelled') ended({ state: 'failed' });
      },
    );
    return () => {
      live = false;
      pairing.cancel();
    };
  }, [pairDevice, attempt]);
  useEffect(() => clear, [clear]);
  const current = attemptInvite(invite, attempt);
  const peer = attemptPeer(found, attempt);
  const seconds = useSecondsLeft(current?.expiresAt ?? 0);
  // A device found beats the code running out.
  const ranOut = peer === null && current !== null && seconds === 0;
  // Remembered here, so the code itself can leave memory.
  if (ranOut && end?.state !== 'expired' && end?.state !== 'linked')
    setEnd(settle(end, { state: 'expired' }));
  useEffect(() => {
    if (ranOut) clear();
  }, [ranOut, clear]);
  const model = addDeviceModel(current, peer, end, seconds, answered);
  // A lost answer is the engine's no: the pairing then ends failed.
  const answer = (accept: boolean) => port.answerPairPeer(accept).catch(() => undefined);
  const linkedId = end?.state === 'linked' ? (end.device?.id ?? null) : null;
  return (
    <AddDeviceDialog
      model={model}
      on={{
        onCancel: nav.close,
        onDone: nav.close,
        onRetry: () => {
          clear();
          setEnd(null);
          setAnswered(false);
          setAttempt(Date.now());
        },
        onOpen: () => {
          if (linkedId === null) nav.close();
          else nav.go({ type: 'openDevice', id: linkedId });
        },
        onConfirm: () => {
          if (answered) return;
          setAnswered(true);
          void answer(true);
        },
        onNotMine: () => {
          if (answered || peer === null) return;
          setAnswered(true);
          void answer(false);
          setEnd(settle(end, { state: 'rejected', kind: deviceKind(peer.os) }));
        },
      }}
    />
  );
}

/** Link a computer by its typed code; closing while it links cancels it. */
export function LinkComputerOverlay() {
  const { linked } = useControllers();
  const nav = useNav();
  const [code, setCode] = useState('');
  const [state, setState] = useState<LinkComputerState>('typing');
  const [device, setDevice] = useState<LinkedDeviceView | null>(null);
  const [ownFingerprint, setOwnFingerprint] = useState<string | null>(null);
  const pairing = useRef<Pairing | null>(null);
  const port = usePort();
  useEffect(() => {
    let live = true;
    port.ownFingerprint().then(
      (fingerprint) => {
        if (live) setOwnFingerprint(fingerprint);
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [port]);
  useEffect(
    () => () => {
      pairing.current?.cancel();
    },
    [],
  );
  return (
    <LinkComputerDialog
      code={code}
      state={state}
      device={device}
      ownFingerprint={ownFingerprint}
      on={{
        onCode: setCode,
        onCancel: nav.close,
        onLink: () => {
          setState('linking');
          pairing.current = linked.linkComputer(code);
          pairing.current.done.then(
            (linkedDevice) => {
              setDevice(linkedDevice);
              setState('linked');
            },
            (reason: unknown) => {
              if (toBridgeError(reason).code !== 'cancelled') setState('failed');
            },
          );
        },
        onOpen: () => {
          if (device === null) nav.close();
          else nav.go({ type: 'openDevice', id: device.id });
        },
      }}
    />
  );
}

/** Forget a linked device; its page closes for Nearby. */
export function ForgetDeviceOverlay({ id }: { readonly id: string }) {
  const { devices, linked } = useControllers();
  const nav = useNav();
  const name = linked.devices.find((d) => d.id === id)?.name ?? id;
  return (
    <ForgetDialog
      name={name}
      on={{
        onCancel: nav.close,
        onForget: () => {
          void devices.forget({ device: id }).then(() => linked.reload());
          nav.go({ type: 'openHub', page: 'nearby' });
        },
      }}
    />
  );
}

/** The clipboard history sheet. */
export function ClipHistoryOverlay() {
  const nav = useNav();
  const { items, clear } = useClipHistory(usePort());
  return <ClipHistorySheet items={items ?? []} on={{ onClear: clear, onClose: nav.close }} />;
}
