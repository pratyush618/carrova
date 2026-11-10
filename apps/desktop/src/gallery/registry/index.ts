import type { GalleryScreen } from '../types';
import { fileScreens } from './file';
import { hubScreens } from './hub';
import { overrideScreens } from './override';
import { receiveScreens } from './receive';
import { sendScreens } from './send';
import { startScreens } from './start';

/**
 * Every Phase 4 screen of the mockup's SCREENS registry, in mockup order.
 * (`xos`, `plugins`, `plugin-perms` are Phase 5; `design` is the mockup's own doc.)
 */
export const SCREENS: readonly GalleryScreen[] = [
  ...startScreens,
  ...sendScreens,
  ...receiveScreens,
  ...fileScreens,
  ...hubScreens,
  ...overrideScreens,
];

/** `#gallery/<id>:<state>` target. */
export interface GalleryRef {
  readonly screen: GalleryScreen;
  readonly state: string;
}

const first = (screen: GalleryScreen): string => screen.states[0]?.id ?? '';

/** Resolves a hash like `recv-progress:paused`; unknown ids fall back to the first screen. */
export function resolve(target: string): GalleryRef | null {
  const [id = '', state = ''] = target.split(':');
  const screen = SCREENS.find((s) => s.id === id) ?? SCREENS[0];
  if (!screen) return null;
  return { screen, state: screen.states.some((s) => s.id === state) ? state : first(screen) };
}

/** The screen `delta` places from `screen`, wrapping around. */
export function neighbour(screen: GalleryScreen, delta: 1 | -1): GalleryScreen {
  const i = SCREENS.indexOf(screen);
  return SCREENS[(i + delta + SCREENS.length) % SCREENS.length] ?? screen;
}

/** The deep-link hash for a screen state. */
export function hashFor(screen: GalleryScreen, state: string): string {
  return `#gallery/${state ? `${screen.id}:${state}` : screen.id}`;
}
