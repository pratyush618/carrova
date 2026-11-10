import { useEffect, useState } from 'react';
import type { NearbyPeer } from '../../bindings/NearbyPeer';

/** Pause between looks while the code screen shows. */
export const NEARBY_POLL_MS = 3_000;

/**
 * Device names of senders nearby offering a code, looked up again every few
 * seconds while `active`. A failed look keeps the last list (it's reassurance only).
 */
export function useNearby(browse: () => Promise<NearbyPeer[]>, active: boolean): readonly string[] {
  const [names, setNames] = useState<readonly string[]>([]);
  useEffect(() => {
    if (!active) return;
    let live = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const look = () => {
      browse()
        .then(
          (peers) => {
            if (live) setNames(peers.map((p) => p.device_name));
          },
          () => undefined,
        )
        .finally(() => {
          if (live) timer = setTimeout(look, NEARBY_POLL_MS);
        });
    };
    look();
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [browse, active]);
  return names;
}
