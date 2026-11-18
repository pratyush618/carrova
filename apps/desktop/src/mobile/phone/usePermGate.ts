import { useState } from 'react';
import type { PhonePerm } from '../../bindings/PhonePerm';
import { usePort } from '../../engine/port';
import { pendingPerm } from './model';
import { permsChanged, usePermSteps } from './usePermSteps';

/**
 * Android permissions, asked only when a feature needing them turns on:
 * `ask` the ones it needs; `asking` is the first not granted yet (back
 * to `null` once all are, read again after each dialog or settings trip).
 * `refused` once Android's dialog left `asking` ungranted: App info is
 * then the way to grant it.
 */
export function usePermGate() {
  const port = usePort();
  // Counted: asking the same perms again reads them again (a read may have failed).
  const [{ perms, attempt }, setAsked] = useState<{
    readonly perms: readonly PhonePerm[];
    readonly attempt: number;
  }>({ perms: [], attempt: 0 });
  const [refused, setRefused] = useState<PhonePerm | null>(null);
  const steps = usePermSteps(perms, attempt);
  const asking = steps === null ? null : pendingPerm(perms, steps);
  return {
    asking,
    refused: asking !== null && asking.perm === refused,
    ask: (next: readonly PhonePerm[]) => {
      setAsked((was) => ({ perms: next, attempt: was.attempt + 1 }));
    },
    close: () => {
      setAsked((was) => ({ perms: [], attempt: was.attempt }));
      setRefused(null);
    },
    allow: () => {
      if (asking === null) return;
      const { perm } = asking;
      port
        .requestPhonePerm(perm)
        .then(
          (step) => {
            if (step !== 'ok') setRefused(perm);
          },
          () => {
            setRefused(perm);
          },
        )
        .finally(permsChanged);
    },
    open: () => {
      if (asking !== null) void port.openPermSettings(asking.perm);
    },
  };
}
