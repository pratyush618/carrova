import { useEffect, useRef } from 'react';
import { usePort } from '../engine/port';
import { openedIntents, type OpenedIntent } from './model';

/**
 * Takes the links the app was opened with (share sheet, tile, notification)
 * at start, when told more wait, and whenever the app comes back to the
 * front, and hands what they ask for to `onIntent`, oldest first.
 */
export function useOpened(onIntent: (intent: OpenedIntent) => void): void {
  const port = usePort();
  const latest = useRef(onIntent);
  useEffect(() => {
    latest.current = onIntent;
  });
  useEffect(() => {
    let live = true;
    let stop: (() => void) | null = null;
    const take = () => {
      port.takeOpened().then(
        (links) => {
          for (const intent of openedIntents(links)) latest.current(intent);
        },
        () => undefined,
      );
    };
    const onShow = () => {
      if (document.visibilityState === 'visible') take();
    };
    void port.onOpened(take).then((unsubscribe) => {
      if (live) stop = unsubscribe;
      else unsubscribe();
    });
    take();
    document.addEventListener('visibilitychange', onShow);
    return () => {
      live = false;
      stop?.();
      document.removeEventListener('visibilitychange', onShow);
    };
  }, [port]);
}
