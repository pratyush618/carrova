import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useHost } from '../../../engine/host';
import { usePort } from '../../../engine/port';
import { toBridgeError } from '../../../engine/errors';
import { useClipHistory } from '../../../flows/ecosystem/useClipHistory';
import type { Pairing } from '../../../flows/ecosystem/useLinkedDevices';
import { ClipHistorySheet } from '../../../screens/clipboard/ClipHistorySheet';
import { AddPhoneDialog } from '../../../screens/devices/AddPhoneDialog';
import { DevicePage } from '../../../screens/devices/DevicePage';
import { ForgetDialog } from '../../../screens/devices/ForgetDialog';
import {
  LinkComputerDialog,
  type LinkComputerState,
} from '../../../screens/devices/LinkComputerDialog';
import type { AddPhoneModel } from '../../../screens/devices/model';
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
        on={{
          onFeature: (feature, state) => {
            linked.setFeature(id, feature, state);
          },
          onRing: () => {
            linked.ring(id).catch(failed);
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

/** How a pairing attempt ended (`null` while it waits). */
type PairEnd =
  | { readonly state: 'linked'; readonly id: string | null; readonly name: string | null }
  | {
      readonly state: 'failed';
    };

/**
 * Add a phone: one invite per attempt; closing the dialog or a new code
 * cancels the one waiting. An invite counts only if it arrived for this attempt.
 */
export function AddPhoneOverlay() {
  const { linked } = useControllers();
  const nav = useNav();
  const invite = useInvite();
  const [attempt, setAttempt] = useState(() => Date.now());
  const [end, setEnd] = useState<PairEnd | null>(null);
  const { pairPhone } = linked;
  useEffect(() => {
    const pairing = pairPhone();
    let live = true;
    pairing.done.then(
      (device) => {
        if (live) setEnd({ state: 'linked', id: device?.id ?? null, name: device?.name ?? null });
      },
      (reason: unknown) => {
        if (live && toBridgeError(reason).code !== 'cancelled') setEnd({ state: 'failed' });
      },
    );
    return () => {
      live = false;
      pairing.cancel();
    };
  }, [pairPhone, attempt]);
  const current = invite !== null && invite.receivedAt >= attempt ? invite : null;
  const expiresAt = current === null ? 0 : current.receivedAt + current.view.expires_in_s * 1000;
  const seconds = useSecondsLeft(expiresAt);
  const waiting: AddPhoneModel =
    current === null
      ? { state: 'starting' }
      : { state: 'waiting', invite: current.view, expiresAt };
  // An expired code wins over a failure: the engine gives up on it once it runs out.
  const model: AddPhoneModel =
    end?.state === 'linked'
      ? end
      : current !== null && seconds === 0
        ? { state: 'expired' }
        : (end ?? waiting);
  const linkedId = end?.state === 'linked' ? end.id : null;
  return (
    <AddPhoneDialog
      model={model}
      on={{
        onCancel: nav.close,
        onDone: nav.close,
        onRetry: () => {
          setEnd(null);
          setAttempt(Date.now());
        },
        onOpen: () => {
          if (linkedId === null) nav.close();
          else nav.go({ type: 'openDevice', id: linkedId });
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
  const [id, setId] = useState<string | null>(null);
  const pairing = useRef<Pairing | null>(null);
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
      on={{
        onCode: setCode,
        onCancel: nav.close,
        onLink: () => {
          setState('linking');
          pairing.current = linked.linkComputer(code);
          pairing.current.done.then(
            (device) => {
              setId(device?.id ?? null);
              setState('linked');
            },
            (reason: unknown) => {
              if (toBridgeError(reason).code !== 'cancelled') setState('failed');
            },
          );
        },
        onOpen: () => {
          if (id === null) nav.close();
          else nav.go({ type: 'openDevice', id });
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
