import { useState } from 'react';
import type { PhonePerm } from '../../bindings/PhonePerm';
import { usePort } from '../../engine/port';
import { pendingPerm } from './model';
import { usePermSteps } from './usePermSteps';

/**
 * Android permissions, asked only when a feature needing them turns on:
 * `ask` the ones it needs; `asking` is the first not granted yet (back
 * to `null` once all are, read again after each settings trip).
 */
export function usePermGate() {
  const port = usePort();
  const [perms, setPerms] = useState<readonly PhonePerm[]>([]);
  const steps = usePermSteps(perms);
  const asking = steps === null ? null : pendingPerm(perms, steps);
  return {
    asking,
    ask: setPerms,
    close: () => {
      setPerms([]);
    },
    open: () => {
      if (asking !== null) void port.openPermSettings(asking.perm);
    },
  };
}
