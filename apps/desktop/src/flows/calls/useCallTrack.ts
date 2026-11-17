import { useCallback, useEffect, useRef, useState } from 'react';
import type { CallView } from '../../bindings/CallView';
import type { BridgeEvent, CarrovaPort } from '../../engine/types';
import { foldCall, holdsLeft, LEFT_GRACE_MS, noCall, type CallTrack } from './callTrack';

/** The call a bridge event shows, if it is a `call_view`. */
function callView(event: BridgeEvent): CallView | null {
  if (event.kind !== 'engine' || event.event.event !== 'call_view') return null;
  // eslint-disable-next-line @typescript-eslint/no-unused-vars -- the tag stays behind
  const { event: tag, ...view } = event.event;
  return view;
}

/**
 * A linked phone's call, from the call when this mounted (`callNow`) and
 * each `call_view` after it. A `left` while the phone rings waits
 * {@link LEFT_GRACE_MS}: a reconnect rings again within it and nothing
 * closes.
 */
export function useCallTrack(port: CarrovaPort): {
  readonly track: CallTrack;
  /** The engine has said where the call stands (until then, `none` means "not known yet"). */
  readonly ready: boolean;
  /** Put a missed call's note away. */
  readonly dismiss: () => void;
} {
  const [track, setTrack] = useState(noCall);
  const [ready, setReady] = useState(false);
  const latest = useRef(noCall);
  useEffect(() => {
    let live = true;
    let heard = false;
    let held: ReturnType<typeof setTimeout> | undefined;
    let stop: (() => void) | null = null;
    const apply = (view: CallView) => {
      latest.current = foldCall(latest.current, view);
      setTrack(latest.current);
      setReady(true);
    };
    const take = (view: CallView) => {
      heard = true;
      // Anything newer (the ringing again) replaces a held `left`.
      clearTimeout(held);
      if (holdsLeft(latest.current, view)) {
        held = setTimeout(() => {
          apply(view);
        }, LEFT_GRACE_MS);
      } else apply(view);
    };
    void port
      .subscribe((event) => {
        const view = callView(event);
        if (view !== null) take(view);
      })
      .then((unsubscribe) => {
        if (!live) {
          unsubscribe();
          return;
        }
        stop = unsubscribe;
        // Asked once listening, so nothing falls between the two.
        port.callNow().then(
          (view) => {
            if (live && !heard) apply(view);
          },
          () => {
            if (live) setReady(true);
          },
        );
      });
    return () => {
      live = false;
      clearTimeout(held);
      stop?.();
    };
  }, [port]);
  const dismiss = useCallback(() => {
    latest.current = noCall;
    setTrack(noCall);
  }, []);
  return { track, ready, dismiss };
}
