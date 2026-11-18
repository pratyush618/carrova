import { useCallback, useEffect, useRef, useState } from 'react';
import type { PhoneSettings } from '../../bindings/PhoneSettings';
import type { CarrovaPort } from '../../engine/types';

/**
 * Settings › Calls and Bluetooth: how a linked phone shows here (`null`
 * until loaded; a change shows at once, and if it can't be saved `failed`
 * says so and the saved ones load again).
 */
export function useCallsSettings(port: CarrovaPort): {
  readonly phone: PhoneSettings | null;
  readonly setPhone: (patch: Partial<PhoneSettings>) => void;
  /** Turn this computer's call audio off (its audio change removed), then `then`. */
  readonly audioOff: (then?: () => void) => void;
  /** The last change could not be made. */
  readonly failed: boolean;
} {
  const [phone, setLoaded] = useState<PhoneSettings | null>(null);
  const [failed, setFailed] = useState(false);
  const saving = useRef(Promise.resolve());
  const load = useCallback(() => {
    port.phoneSettings().then(setLoaded, () => undefined);
  }, [port]);
  useEffect(load, [load]);
  const setPhone = useCallback(
    (patch: Partial<PhoneSettings>) => {
      if (phone === null) return;
      const next = { ...phone, ...patch };
      setLoaded(next);
      setFailed(false);
      // One save at a time, in order: an earlier one can't land after (and undo) a later one.
      saving.current = saving.current
        .then(() => port.setPhoneSettings(next))
        .catch(() => {
          setFailed(true);
          load();
        });
    },
    [phone, port, load],
  );
  const audioOff = useCallback(
    (then?: () => void) => {
      setFailed(false);
      port.setCallAudioConfig(false).then(then, () => {
        setFailed(true);
      });
    },
    [port],
  );
  return { phone, setPhone, audioOff, failed };
}
