import { useCallback, useState } from 'react';
import type { LinkedDeviceView } from '../engine/types';

/** What the last send said: it went, or the computer didn't answer. */
export interface SendNote {
  readonly kind: 'clipSent' | 'linkSent' | 'actionFailed';
  readonly name: string;
}

/** Each note's message. */
export const NOTE_KEY = {
  clipSent: 'ecosystem.device.clipSent',
  linkSent: 'mobile.pick.linkSent',
  actionFailed: 'ecosystem.device.actionFailed',
} as const;

/** Run `send` to `device`; note `sent` when it went (if it names one), and a failure. */
export type SendTo = (
  device: LinkedDeviceView,
  send: (id: string) => Promise<void>,
  sent: SendNote['kind'] | null,
) => void;

/** Sends started from the phone, and the note the last one left. */
export function useSendNote() {
  const [note, setNote] = useState<SendNote | null>(null);
  const sendTo = useCallback<SendTo>((device, send, sent) => {
    send(device.id).then(
      () => {
        if (sent !== null) setNote({ kind: sent, name: device.name });
      },
      () => {
        setNote({ kind: 'actionFailed', name: device.name });
      },
    );
  }, []);
  const dismiss = useCallback(() => {
    setNote(null);
  }, []);
  return { note, sendTo, dismiss };
}
