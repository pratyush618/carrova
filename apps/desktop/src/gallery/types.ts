import type { ReactNode } from 'react';
import type { View } from '../app/nav/navMachine';

/** One named state of a screen (deep link: `#gallery/<screen>:<state>`). */
export interface GalleryState {
  readonly id: string;
  readonly label: string;
}

/** What a gallery entry renders: the screen, plus any overlays above it. */
export interface GalleryRender {
  readonly base: ReactNode;
  /** Overrides the screen's `view` for this state. */
  readonly view?: View;
  readonly overlays?: readonly ReactNode[];
}

/** A screen of the mockup's SCREENS registry, rendered from fixtures. */
export interface GalleryScreen {
  readonly id: string;
  readonly group: string;
  readonly title: string;
  /** Where the screen sits in navigation (drives chrome: nav highlight, step dots). */
  readonly view: View;
  readonly states: readonly GalleryState[];
  readonly render: (state: string) => GalleryRender;
}

/** Builds `[{id,label}]` from `[id, label]` pairs. */
export const states = (...pairs: readonly (readonly [string, string])[]): GalleryState[] =>
  pairs.map(([id, label]) => ({ id, label }));
