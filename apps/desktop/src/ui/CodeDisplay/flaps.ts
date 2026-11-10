/** Timing of the split-flap reveal, from the mockup. */
export const FLAP_TICK_MS = 60;
const FIRST_SETTLE_MS = 350;
const STAGGER_MS = 70;

const LETTERS = 'abcdefghijklmnopqrstuvwxyz';
const DIGITS = '0123456789';

/** When tile `index` stops spinning and shows its real character. */
export function settleAt(index: number): number {
  return FIRST_SETTLE_MS + index * STAGGER_MS;
}

/** Total reveal time for `tiles` tiles. */
export function revealDuration(tiles: number): number {
  return tiles === 0 ? 0 : settleAt(tiles - 1);
}

/**
 * What tile `index` shows `elapsed` ms into the reveal. While spinning it
 * cycles through characters of the same kind (digits stay digits), derived
 * from the tick so rendering stays pure.
 */
export function flapFace(target: string, index: number, elapsed: number): string {
  if (elapsed >= settleAt(index)) return target;
  const pool = /\d/.test(target) ? DIGITS : LETTERS;
  const tick = Math.floor(elapsed / FLAP_TICK_MS);
  return pool.charAt((tick * 7 + index * 13) % pool.length);
}
