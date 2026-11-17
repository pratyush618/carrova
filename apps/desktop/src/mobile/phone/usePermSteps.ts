import { useEffect, useState } from 'react';
import type { PermStep } from '../../bindings/PermStep';
import type { PhonePerm } from '../../bindings/PhonePerm';
import { usePort } from '../../engine/port';

/** Fired on `document` after Android's permission dialog: it doesn't hide the app. */
const PERMS_CHANGED = 'carrova-perms-changed';

/** Have every {@link usePermSteps} read its steps again. */
export const permsChanged = () => document.dispatchEvent(new Event(PERMS_CHANGED));

/**
 * The step each of `perms` needs (`null` until read), read again whenever
 * the app comes back to the front (the user may have granted one in
 * Android's settings meanwhile) or after {@link permsChanged}.
 */
export function usePermSteps(perms: readonly PhonePerm[]): readonly PermStep[] | null {
  const port = usePort();
  // Tagged with the perms they were read for: other perms' steps read as unread.
  const [read, setRead] = useState<{ key: string; steps: readonly PermStep[] } | null>(null);
  const key = perms.join(',');
  useEffect(() => {
    let live = true;
    const list = key === '' ? [] : (key.split(',') as PhonePerm[]);
    const load = () => {
      Promise.all(list.map((perm) => port.phonePermStep(perm))).then(
        (next) => {
          if (live) setRead({ key, steps: next });
        },
        // Unreadable: keep what shows; the next return to the app tries again.
        () => undefined,
      );
    };
    const back = () => {
      if (document.visibilityState === 'visible') load();
    };
    load();
    document.addEventListener('visibilitychange', back);
    document.addEventListener(PERMS_CHANGED, load);
    return () => {
      live = false;
      document.removeEventListener('visibilitychange', back);
      document.removeEventListener(PERMS_CHANGED, load);
    };
  }, [port, key]);
  return read?.key === key ? read.steps : null;
}
