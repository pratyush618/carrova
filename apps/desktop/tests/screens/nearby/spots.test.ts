import { describe, expect, it } from 'vitest';
import type { Ring } from '@/engine/types';
import { spot } from '@/screens/nearby/spots';

// Conservative device pill: the widest fixture pill (163px, "new-mbp") by the real height (52px).
const PILL = { width: 164, height: 52 };
// Pills clear each other by this much, not just touching: spots are rounded to whole pixels and real pill sizes vary a little with the font (the obstacles below are measured, so no gap).
const GAP = 6;
// Keep in sync with nearby.css: `--cy` (the centre's share of the field height) and the inner `--k` floor.
const CY = 0.38;
const INNER_FLOOR = 0.95;

// Obstacles, measured from e2e at 1100x520 / 720x560 / 1440x900 (nearby.spec.ts asserts the real layout):
// the largest open card (a known device, three fact rows) sits at 20,20; the visibility pill is 298x38, centred,
// 22px above the page foot (`pillTop`); this computer's disc + label is 87x90 around the centre.
const CARD = { x: 20, y: 20, width: 288, height: 200 };
const VISIBILITY = { width: 298, height: 38 };
const CENTRE = { width: 87, height: 90 };
const FIELDS = [
  { name: '720x560', width: 508, height: 520, pillTop: 448 },
  { name: '1100x520', width: 888, height: 520, pillTop: 408 },
  { name: '1440x900', width: 1228, height: 848, pillTop: 788 },
];

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

type Field = (typeof FIELDS)[number];

const centred = (x: number, y: number, size: { width: number; height: number }): Box => ({
  x: x - size.width / 2,
  y: y - size.height / 2,
  ...size,
});

const apart = (a: Box, b: Box, gap: number) =>
  a.x + a.width + gap <= b.x ||
  b.x + b.width + gap <= a.x ||
  a.y + a.height + gap <= b.y ||
  b.y + b.height + gap <= a.y;

/** The device pills of `inner` + `outer` devices on `field`, in field coordinates. */
function pills(field: Field, inner: number, outer: number): Box[] {
  const u = Math.min(1, (field.width / 2 - 96) / 290); // --u in nearby.css
  const on = (ring: Ring, count: number, k: number) =>
    Array.from({ length: count }, (_, i) => {
      const { dx, dy } = spot(ring, i, count);
      return centred(field.width / 2 + dx * k, CY * field.height + dy, PILL);
    });
  return [...on('inner', inner, Math.max(u, INNER_FLOOR)), ...on('outer', outer, u)];
}

/** What the pills must stay clear of: the card, the visibility pill and this computer. */
function obstacles(field: Field): Box[] {
  return [
    CARD,
    { x: (field.width - VISIBILITY.width) / 2, y: field.pillTop, ...VISIBILITY },
    centred(field.width / 2, CY * field.height, CENTRE),
  ];
}

describe('Nearby spots', () => {
  // Guaranteed: up to 5 devices, at most 2 inner and at most 4 outer, in any split. A 3rd inner (its spot meets the card), a 5th outer, or 6+ devices is the open density follow-up.
  it('keep every pill clear of every other, the card, the visibility pill and this computer for up to 5 devices, at most 2 inner and 4 outer', () => {
    for (const field of FIELDS) {
      for (let inner = 0; inner <= 2; inner += 1) {
        for (let outer = 0; outer <= Math.min(4, 5 - inner); outer += 1) {
          const all = pills(field, inner, outer);
          const fixed = obstacles(field);
          const label = `${String(inner)} inner + ${String(outer)} outer at ${field.name}`;
          for (const [i, a] of all.entries()) {
            expect(
              all.slice(i + 1).every((b) => apart(a, b, GAP)) && fixed.every((o) => apart(a, o, 0)),
              label,
            ).toBe(true);
            expect(a.x >= 0 && a.x + a.width <= field.width && a.y >= 0, label).toBe(true);
          }
        }
      }
    }
  });
});
