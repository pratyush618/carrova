import type { Ring } from '../../engine/types';

/** A fixed place for one device on the map. */
export interface Slot {
  /** Distance from this computer in field units (`--u`, 1px on a roomy field). */
  readonly radius: number;
  /** Degrees clockwise from the inline end (right), so 90 is straight down. */
  readonly angle: number;
  /** The narrowest field (px) where a pill here clears the rest. */
  readonly minField: number;
}

/** The narrowest field: the smallest window (720px) minus the sidebar. */
export const MIN_FIELD = 508;

export const RING_ORDER: readonly Ring[] = ['inner', 'outer'];

/** Inner devices sit between the first two orbs; outer ones on the third orb or, roomier, the fourth. */
const NEAR = 170;
const OUTER = 290;
const FAR = 420;

/**
 * Each ring's slots, in the order they fill: every slot open at a field
 * width, and the chip, clear each other (6px), the open card (top start),
 * the visibility pill (bottom centre), this computer and the page edges,
 * and inner slots sit nearer than outer ones; tests/screens/nearby/spots.test.ts
 * proves it for every width. Wider fields open more slots; the fill order
 * alternates sides where it can, skipping slots not open yet.
 */
export const SLOTS: Readonly<Record<Ring, readonly Slot[]>> = {
  inner: [
    { radius: NEAR, angle: -15, minField: MIN_FIELD },
    { radius: NEAR, angle: 150, minField: MIN_FIELD },
    { radius: NEAR, angle: -80, minField: 724 },
    { radius: NEAR, angle: 20, minField: MIN_FIELD },
    { radius: NEAR, angle: 70, minField: MIN_FIELD },
    { radius: NEAR, angle: -170, minField: 1116 },
  ],
  outer: [
    { radius: OUTER, angle: -30, minField: MIN_FIELD },
    { radius: OUTER, angle: 140, minField: MIN_FIELD },
    { radius: OUTER, angle: -65, minField: 608 },
    { radius: FAR, angle: 135, minField: MIN_FIELD },
    { radius: FAR, angle: 55, minField: 748 },
    { radius: FAR, angle: 165, minField: 976 },
    { radius: FAR, angle: -40, minField: 808 },
    { radius: OUTER, angle: -140, minField: 1224 },
    { radius: FAR, angle: 35, minField: 852 },
    { radius: OUTER, angle: -100, minField: 880 },
    { radius: FAR, angle: 20, minField: 956 },
    { radius: FAR, angle: -5, minField: 1000 },
  ],
};

/** The "+N more" chip's place, under this computer, open on every field. */
export const MORE: Slot = { radius: OUTER, angle: 85, minField: MIN_FIELD };

/** The field is wider than tall: devices sit on flattened orbits (their height is never shrunk by `--u`, only their width). */
const FLATTEN = 0.6;

/** Where a pill in `slot` sits, from the centre. */
export function spot({ radius, angle }: Slot): { dx: number; dy: number } {
  const radians = (angle * Math.PI) / 180;
  return {
    dx: Math.round(radius * Math.cos(radians)),
    dy: Math.round(radius * Math.sin(radians) * FLATTEN),
  };
}
