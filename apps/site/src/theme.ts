import { useSyncExternalStore } from 'react';
import { flushSync } from 'react-dom';

/** What the visitor picked; `system` follows their OS, like the app's Appearance setting. */
export type ThemePref = 'system' | 'light' | 'dark';
export type Scheme = 'light' | 'dark';

// Same key and resolution as the pre-paint script in index.html.
const KEY = 'carrova-theme';
const media = window.matchMedia('(prefers-color-scheme: dark)');
const listeners = new Set<() => void>();

export function resolveScheme(pref: ThemePref, systemDark: boolean): Scheme {
  if (pref === 'system') return systemDark ? 'dark' : 'light';
  return pref;
}

function readPref(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    return v === 'light' || v === 'dark' ? v : 'system';
  } catch {
    return 'system';
  }
}

let pref = readPref();
let scheme = resolveScheme(pref, media.matches);

/** Page background per scheme, for the browser's toolbar (index.html uses the same values). */
const TOOLBAR: Record<Scheme, string> = { light: '#FFFFFF', dark: '#0D0E11' };

function paint() {
  document.documentElement.dataset['theme'] = scheme;
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', TOOLBAR[scheme]);
}

// The OS may have switched since the pre-paint script ran.
paint();

function apply() {
  scheme = resolveScheme(pref, media.matches);
  paint();
  for (const l of listeners) l();
}

media.addEventListener('change', apply);

const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)');

/**
 * Picks a theme. With `from` (the control's centre), the new theme spreads out
 * from it in a circle; without View Transitions or under reduced motion, it just switches.
 */
export function setThemePref(next: ThemePref, from?: { readonly x: number; readonly y: number }) {
  pref = next;
  try {
    if (next === 'system') localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, next);
  } catch {
    // Storage blocked: the choice lasts for this visit only.
  }
  const same = resolveScheme(next, media.matches) === scheme;
  if (same || from === undefined || reducedMotion.matches || !('startViewTransition' in document)) {
    apply();
    return;
  }
  const change = document.startViewTransition(() => {
    flushSync(apply);
  });
  const radius = Math.hypot(
    Math.max(from.x, window.innerWidth - from.x),
    Math.max(from.y, window.innerHeight - from.y),
  );
  // A skipped transition rejects `ready`; the theme is already applied by then.
  change.ready.then(
    () => {
      document.documentElement.animate(
        {
          clipPath: [
            `circle(0px at ${String(from.x)}px ${String(from.y)}px)`,
            `circle(${String(radius)}px at ${String(from.x)}px ${String(from.y)}px)`,
          ],
        },
        {
          duration: 650,
          easing: 'cubic-bezier(0.16, 1, 0.3, 1)',
          pseudoElement: '::view-transition-new(root)',
        },
      );
    },
    () => undefined,
  );
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

export const useThemePref = (): ThemePref => useSyncExternalStore(subscribe, () => pref);
export const useScheme = (): Scheme => useSyncExternalStore(subscribe, () => scheme);
