import { useCallback, useSyncExternalStore } from 'react';

const PREFIX = '#gallery';

const subscribe = (onChange: () => void) => {
  window.addEventListener('hashchange', onChange);
  return () => {
    window.removeEventListener('hashchange', onChange);
  };
};

/** The gallery target after `#gallery/` in the URL, and a setter that pushes it. */
export function useHashTarget(): readonly [string, (hash: string) => void] {
  const target = useSyncExternalStore(subscribe, () =>
    window.location.hash.slice(PREFIX.length + 1),
  );
  const go = useCallback((hash: string) => {
    window.location.hash = hash;
  }, []);
  return [target, go];
}

/** True when the URL asks for the gallery. */
export const wantsGallery = (): boolean => window.location.hash.startsWith(PREFIX);
