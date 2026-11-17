import { useCallback, useEffect, useState } from 'react';
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
  /** The last change could not be made. */
  readonly failed: boolean;
} {
  const [phone, setLoaded] = useState<PhoneSettings | null>(null);
  const [failed, setFailed] = useState(false);
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
      port.setPhoneSettings(next).catch(() => {
        setFailed(true);
        load();
      });
    },
    [phone, port, load],
  );
  return { phone, setPhone, failed };
}
