import type { Ring } from '../../engine/types';

/** Ring radius in field units (`--u`, 1px on a roomy field; inner = answering now), and where on it the first device sits.
 * The starts are 50° apart so, for 1–3 inner and 1–5 outer devices, no two ever share an angle (60° would, at 1 inner and 4 outer). */
const RINGS: Readonly<Record<Ring, { readonly radius: number; readonly start: number }>> = {
  inner: { radius: 170, start: 20 },
  outer: { radius: 290, start: -30 },
};

export const RING_ORDER: readonly Ring[] = ['inner', 'outer'];

/** The field is wider than tall: devices sit on a flattened orbit (its height is never shrunk by `--u`, only its width). */
const FLATTEN = 0.6;

/**
 * Devices share an arc of the ring, not the whole circle, so the part under
 * the card (top left) and under the visibility pill (bottom) stays free.
 */
const SWEEP = 240;

/** The angle (degrees) of the `index`-th of `count` devices on `ring`. */
export function angle(ring: Ring, index: number, count: number): number {
  return RINGS[ring].start + (index * SWEEP) / count;
}

/** Where the `index`-th of `count` devices on `ring` sits, from the centre. */
export function spot(ring: Ring, index: number, count: number): { dx: number; dy: number } {
  const radians = (angle(ring, index, count) * Math.PI) / 180;
  return {
    dx: Math.round(RINGS[ring].radius * Math.cos(radians)),
    dy: Math.round(RINGS[ring].radius * Math.sin(radians) * FLATTEN),
  };
}
