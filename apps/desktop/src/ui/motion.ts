import { createContext, useCallback, useContext, useSyncExternalStore } from 'react';

const canMatch = (): boolean =>
  typeof window !== 'undefined' && typeof window.matchMedia === 'function';

/** Live result of a CSS media query (false where `matchMedia` is unavailable). */
export function useMediaQuery(query: string): boolean {
  const subscribe = useCallback(
    (onChange: () => void) => {
      if (!canMatch()) return () => undefined;
      const list = window.matchMedia(query);
      list.addEventListener('change', onChange);
      return () => {
        list.removeEventListener('change', onChange);
      };
    },
    [query],
  );
  return useSyncExternalStore(
    subscribe,
    () => canMatch() && window.matchMedia(query).matches,
    () => false,
  );
}

/** The user's own "reduce motion" switch, on top of the OS setting. */
export const ReduceMotionOverride = createContext(false);

/**
 * True when decorative motion must stop: the OS asks for it or the user
 * turned it on in Carrova. Functional updates (progress) still happen.
 */
export function useReducedMotion(): boolean {
  const os = useMediaQuery('(prefers-reduced-motion: reduce)');
  const user = useContext(ReduceMotionOverride);
  return os || user;
}
