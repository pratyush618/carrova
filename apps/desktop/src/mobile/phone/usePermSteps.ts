import { useEffect, useState } from 'react';
import type { PermStep } from '../../bindings/PermStep';
import type { PhonePerm } from '../../bindings/PhonePerm';
import { usePort } from '../../engine/port';

/**
 * The step each of `perms` needs (`null` until read), read again whenever
 * the app comes back to the front (the user may have granted one in
 * Android's settings meanwhile) or `attempt` changes.
 */
export function usePermSteps(perms: readonly PhonePerm[], attempt = 0): readonly PermStep[] | null {
  const port = usePort();
  // Tagged with the perms they were read for: other perms' steps read as unread.
  const [read, setRead] = useState<{ key: string; steps: readonly PermStep[] } | null>(null);
  const key = perms.join(',');
  useEffect(() => {
    let live = true;
    // Reads may finish out of order: only the latest one shows.
    let latest = 0;
    const list = key === '' ? [] : (key.split(',') as PhonePerm[]);
    const load = () => {
      const at = ++latest;
      Promise.all(list.map((perm) => port.phonePermStep(perm))).then(
        (next) => {
          if (live && at === latest) setRead({ key, steps: next });
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
    return () => {
      live = false;
      document.removeEventListener('visibilitychange', back);
    };
  }, [port, key, attempt]);
  return read?.key === key ? read.steps : null;
}
