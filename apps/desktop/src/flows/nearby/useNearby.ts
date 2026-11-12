import { useEffect, useState } from 'react';

/** Pause between looks while a nearby list shows. */
export const NEARBY_POLL_MS = 3_000;

/**
 * What `browse` finds nearby, looked up again every few seconds while
 * `active`. A failed look keeps the last list (nearby is best effort).
 */
export function useNearby<T>(browse: () => Promise<T[]>, active: boolean): readonly T[] {
  const [found, setFound] = useState<readonly T[]>([]);
  useEffect(() => {
    if (!active) return;
    let live = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const look = () => {
      browse()
        .then(
          (next) => {
            if (live) setFound(next);
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
  return found;
}
