import { describe, expect, it } from 'vitest';
import type { Ring } from '@/engine/types';
import { spot } from '@/screens/nearby/spots';

// Conservative pill: the widest fixture pill (163px, "new-mbp") by the real height measured in e2e (52px).
const PILL = { width: 164, height: 52 };
// Clear by this much, not just touching: spots are rounded to whole pixels and the real pill height varies a little with the font.
const GAP = 6;
// Field widths: the narrowest supported (508px: --u 0.545) and a roomy one; inner spots shrink by --k = max(--u, 0.85).
const FIELDS = [0.545, 1];

interface Pill {
  x: number;
  y: number;
}

function pills(inner: number, outer: number, u: number): Pill[] {
  const on = (ring: Ring, count: number, k: number) =>
    Array.from({ length: count }, (_, i) => {
      const { dx, dy } = spot(ring, i, count);
      return { x: dx * k, y: dy };
    });
  return [...on('inner', inner, Math.max(u, 0.85)), ...on('outer', outer, u)];
}

const overlapping = (a: Pill, b: Pill) =>
  Math.abs(a.x - b.x) < PILL.width + GAP && Math.abs(a.y - b.y) < PILL.height + GAP;

describe('Nearby spots', () => {
  // Guaranteed: up to 5 devices with at most 4 outer, in any split (0–3 inner). A 5th outer, or 6+ devices, is the open density follow-up.
  it('keep every pill clear of every other for up to 5 devices, at most 4 of them outer', () => {
    for (const u of FIELDS) {
      for (let inner = 0; inner <= 3; inner += 1) {
        for (let outer = 0; outer <= Math.min(4, 5 - inner); outer += 1) {
          const all = pills(inner, outer, u);
          const clash = all.some((a, i) => all.slice(i + 1).some((b) => overlapping(a, b)));
          expect(clash, `${String(inner)} inner + ${String(outer)} outer at u=${String(u)}`).toBe(
            false,
          );
        }
      }
    }
  });
});
