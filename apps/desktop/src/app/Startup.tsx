import { useEffect, useState } from 'react';
import { usePort } from '../engine/port';
import { firstRun, returning, type Nav } from './nav/navMachine';
import { Shell } from './shell/Shell';

/**
 * Picks where the app opens: Welcome while error-report consent was never
 * asked (it asks there), else Home. Nothing renders until that's known.
 */
export function Startup() {
  const port = usePort();
  const [initial, setInitial] = useState<Nav | null>(null);
  useEffect(() => {
    let live = true;
    port.errorReportConsent().then(
      (consent) => {
        if (live) setInitial(consent === 'unset' ? firstRun : returning);
      },
      () => {
        if (live) setInitial(firstRun);
      },
    );
    return () => {
      live = false;
    };
  }, [port]);
  return initial === null ? null : <Shell initial={initial} />;
}
