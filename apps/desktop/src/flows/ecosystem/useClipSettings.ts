import { useCallback, useEffect, useState } from 'react';
import type { CarrovaPort, ClipSettings } from '../../engine/types';

/**
 * The user's clipboard settings (`null` until loaded). A change shows at
 * once; if it cannot be saved, `failed` says so and the saved ones are
 * loaded again.
 */
export function useClipSettings(port: CarrovaPort): {
  readonly clip: ClipSettings | null;
  readonly setClip: (patch: Partial<ClipSettings>) => void;
  /** The last change could not be saved. */
  readonly failed: boolean;
} {
  const [clip, setLoaded] = useState<ClipSettings | null>(null);
  const [failed, setFailed] = useState(false);
  const load = useCallback(() => {
    port.clipSettings().then(setLoaded, () => undefined);
  }, [port]);
  useEffect(load, [load]);
  const setClip = useCallback(
    (patch: Partial<ClipSettings>) => {
      if (clip === null) return;
      const next = { ...clip, ...patch };
      setLoaded(next);
      setFailed(false);
      port.setClipSettings(next).catch(() => {
        setFailed(true);
        load();
      });
    },
    [clip, port, load],
  );
  return { clip, setClip, failed };
}
