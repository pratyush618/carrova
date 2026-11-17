import { useEffect, useState } from 'react';
import type { CallView } from '../../bindings/CallView';
import type { CallWindowView } from '../../bindings/CallWindowView';
import type { CarrovaPort } from '../../engine/types';

/** How often an active call's timer is looked up again. */
const TICK_MS = 1000;

/**
 * What the call windows show of `view` (`callWindow`, so the timer's
 * format stays in Rust), again every second while the call is active.
 * `null` until the first answer.
 */
export function useCallWindowView(port: CarrovaPort, view: CallView): CallWindowView | null {
  const [shown, setShown] = useState<CallWindowView | null>(null);
  useEffect(() => {
    let live = true;
    const look = () => {
      port.callWindow(view, Date.now()).then(
        (next) => {
          if (live) setShown(next);
        },
        () => undefined,
      );
    };
    look();
    const timer = view.kind === 'active' ? setInterval(look, TICK_MS) : undefined;
    return () => {
      live = false;
      clearInterval(timer);
    };
  }, [port, view]);
  return shown;
}
