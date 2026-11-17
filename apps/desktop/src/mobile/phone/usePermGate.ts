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
  // Counted: asking the same perms again reads them again (a read may have failed).
  const [{ perms, attempt }, setAsked] = useState<{
    readonly perms: readonly PhonePerm[];
    readonly attempt: number;
  }>({ perms: [], attempt: 0 });
  const steps = usePermSteps(perms, attempt);
  const asking = steps === null ? null : pendingPerm(perms, steps);
  return {
    asking,
    ask: (next: readonly PhonePerm[]) => {
      setAsked((was) => ({ perms: next, attempt: was.attempt + 1 }));
    },
    close: () => {
      setAsked((was) => ({ perms: [], attempt: was.attempt }));
    },
    open: () => {
      if (asking !== null) void port.openPermSettings(asking.perm);
    },
  };
}
