import { useCallback, useEffect, useRef, useState } from 'react';
import { useControllers } from '../../app/controllers/Controllers';
import { toBridgeError } from '../../engine/errors';
import { useHost } from '../../engine/host';
import { usePort } from '../../engine/port';
import type { LinkedDeviceView } from '../../engine/types';
import type { Pairing } from '../../flows/ecosystem/useLinkedDevices';

/** Where adding a computer stands. */
export type PhonePairStep =
  /** Checking that the phone is on a network first. */
  | { readonly kind: 'checking' }
  /** Not on any LAN: linking cannot work. */
  | { readonly kind: 'offline' }
  /** Scan the code, or type it instead; `scanFailed`: the last scan could not read one. */
  | { readonly kind: 'add'; readonly scanFailed: boolean }
  | { readonly kind: 'typing' }
  | { readonly kind: 'linking' }
  /** `device`: the computer that linked, when the list names it. */
  | { readonly kind: 'linked'; readonly device: LinkedDeviceView | null }
  | { readonly kind: 'failed' };

/**
 * Adding a computer from the phone: checks the LAN on entering and on each
 * retry, scans (or takes a typed code) and links through the shared
 * linked-devices controller; once linked, asks to show notifications.
 * Leaving cancels a link still under way.
 */
export function usePhonePairing() {
  const port = usePort();
  const host = useHost();
  const { linked } = useControllers();
  const [step, setStep] = useState<PhonePairStep>({ kind: 'checking' });
  const [ownFingerprint, setOwnFingerprint] = useState<string | null>(null);
  const pairing = useRef<Pairing | null>(null);

  // A failed check never blocks linking: the link itself then says what went wrong.
  const probe = useCallback(() => {
    port.lanAvailable().then(
      (lan) => {
        setStep(lan ? { kind: 'add', scanFailed: false } : { kind: 'offline' });
      },
      () => {
        setStep({ kind: 'add', scanFailed: false });
      },
    );
  }, [port]);
  useEffect(probe, [probe]);
  useEffect(() => {
    port.ownFingerprint().then(setOwnFingerprint, () => undefined);
  }, [port]);
  useEffect(
    () => () => {
      pairing.current?.cancel();
    },
    [],
  );

  const { linkComputer } = linked;
  const link = useCallback(
    (ticket: string) => {
      setStep({ kind: 'linking' });
      const current = linkComputer(ticket);
      pairing.current = current;
      current.done.then(
        (device) => {
          setStep({ kind: 'linked', device });
          // The link notification needs it; refused, everything else still works.
          host.askNotifications?.().catch(() => undefined);
        },
        (reason: unknown) => {
          if (toBridgeError(reason).code !== 'cancelled') setStep({ kind: 'failed' });
        },
      );
    },
    [linkComputer, host],
  );
  const scan = useCallback(() => {
    port.scanQr().then(
      (uri) => {
        if (uri !== null) link(uri);
      },
      // No camera, or not a Carrova code: nothing was linked, so scan again or type it.
      () => {
        setStep({ kind: 'add', scanFailed: true });
      },
    );
  }, [port, link]);
  const retry = useCallback(() => {
    setStep({ kind: 'checking' });
    probe();
  }, [probe]);
  const type = useCallback(() => {
    setStep({ kind: 'typing' });
  }, []);
  return { step, ownFingerprint, scan, type, link, retry };
}
