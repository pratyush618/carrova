import { useCallback, useEffect, useState } from 'react';
import { useLink } from '../../app/state/EngineState';
import type { CarrovaPort, ClipItemView } from '../../engine/types';

/** The received clips, newest first: loaded on open and again as clips arrive. */
export function useClipHistory(port: CarrovaPort): {
  readonly items: readonly ClipItemView[] | null;
  readonly clear: () => void;
} {
  const [items, setItems] = useState<readonly ClipItemView[] | null>(null);
  const { clips } = useLink();
  const load = useCallback(() => {
    void port.clipHistory().then(
      (history) => {
        setItems(history.items);
      },
      () => {
        setItems((now) => now ?? []);
      },
    );
  }, [port]);
  useEffect(load, [load, clips]);
  const clear = useCallback(() => {
    void port.clearClipHistory().finally(load);
  }, [port, load]);
  return { items, clear };
}
