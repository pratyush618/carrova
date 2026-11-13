import { describe, expect, it } from 'vitest';
import type { Ring } from '@/engine/types';
import { MIN_FIELD, MORE, RING_ORDER, SLOTS, spot, type Slot } from '@/screens/nearby/spots';

// The widest device pill (nearby.css caps it at 164px) by the real height (52px); the "+N more" chip is smaller.
const PILL = { width: 164, height: 52 };
// Pills clear each other by this much, not just touching: spots are rounded to whole pixels and real pill sizes vary a little with the font (the obstacles below are measured, so no gap).
const GAP = 6;
// Keep in sync with nearby.css: `--cy` (the centre's share of the field height) and the inner `--k` floor.
const CY = 0.38;
const INNER_FLOOR = 0.95;

// Obstacles, measured from e2e at 1100x520 / 720x560 / 1440x900 (nearby.spec.ts asserts the real layout):
// the largest open card (a known device, three fact rows) sits at 20,20; the visibility pill is capped at 300x38 (nearby.css; a long computer name reaches it), centred,
// 22px above the page foot (`pillTop`); this computer's disc + label is 87x90 around the centre.
const CARD = { x: 20, y: 20, width: 288, height: 200 };
const VISIBILITY = { width: 300, height: 38, below: 22 };
const CENTRE = { width: 87, height: 90 };

interface Field {
  name: string;
  width: number;
  /** The field is never shorter than 520px; the page under it can be. */
  height: number;
  pillTop: number;
}

// The windows the app is checked at (each minus the 212px sidebar), and every width from the narrowest
// field up at the shortest window (720x520 is the minimum; a taller window only moves the centre away from the
// card and the pill away from the centre).
const FIELDS: Field[] = [
  { name: '720x560', width: 508, height: 520, pillTop: 448 },
  { name: '1100x520', width: 888, height: 520, pillTop: 408 },
  { name: '1440x900', width: 1228, height: 848, pillTop: 788 },
  ...Array.from({ length: 374 }, (_, i) => {
    const width = MIN_FIELD + 4 * i;
    return { name: `${String(width)}x520`, width, height: 520, pillTop: 408 };
  }),
];

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

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

/** Where a pill in `slot` sits on `field`, and how far its middle is from this computer's. */
function pill(field: Field, slot: Slot, ring: Ring | null) {
  const u = Math.min(1, (field.width / 2 - 96) / 290); // --u in nearby.css
  const k = ring === 'inner' ? Math.max(u, INNER_FLOOR) : u;
  const { dx, dy } = spot(slot);
  return {
    box: centred(field.width / 2 + dx * k, CY * field.height + dy, PILL),
    away: Math.hypot(dx * k, dy),
  };
}

/** Every slot open on `field`, by ring; the chip's ring is `null`. */
function open(field: Field) {
  return [
    ...RING_ORDER.flatMap((ring) =>
      SLOTS[ring]
        .map((slot, i) => ({
          name: `${ring} ${String(i)}`,
          ring,
          ...pill(field, slot, ring),
          slot,
        }))
        .filter(({ slot }) => slot.minField <= field.width),
    ),
    { name: 'more', ring: null, ...pill(field, MORE, null), slot: MORE },
  ];
}

/** What the pills must stay clear of: the card, the visibility pill and this computer. */
function obstacles(field: Field): Box[] {
  return [
    CARD,
    { x: (field.width - VISIBILITY.width) / 2, y: field.pillTop, ...VISIBILITY },
    centred(field.width / 2, CY * field.height, CENTRE),
  ];
}

describe('Nearby slots', () => {
  // Any number of devices fits: every slot open at a width, and the chip, clear everything at once.
  it('clear each other, the card, the visibility pill and this computer, all open at once, at every field width', () => {
    for (const field of FIELDS) {
      const all = open(field);
      const fixed = obstacles(field);
      const foot = field.pillTop + VISIBILITY.height + VISIBILITY.below;
      for (const [i, { name, box }] of all.entries()) {
        const label = `${name} at ${field.name}`;
        for (const other of all.slice(i + 1))
          expect(apart(box, other.box, GAP), `${label} meets ${other.name}`).toBe(true);
        expect(
          fixed.every((o) => apart(box, o, 0)),
          `${label} meets an obstacle`,
        ).toBe(true);
        expect(
          box.x >= 0 &&
            box.x + box.width <= field.width &&
            box.y >= 0 &&
            box.y + box.height <= foot,
          `${label} leaves the page`,
        ).toBe(true);
      }
    }
  });

  it('keep devices answering now nearer this computer than the rest, at every field width', () => {
    for (const field of FIELDS) {
      const all = open(field);
      const far = (ring: Ring) => all.filter((s) => s.ring === ring).map((s) => s.away);
      expect(Math.max(...far('inner')), field.name).toBeLessThan(Math.min(...far('outer')));
    }
  });

  it('give every ring a slot at the narrowest field, so the selected device always shows', () => {
    for (const ring of RING_ORDER)
      expect(SLOTS[ring].some((slot) => slot.minField <= MIN_FIELD)).toBe(true);
    expect(MORE.minField).toBe(MIN_FIELD);
  });

  it('open more slots as the field widens', () => {
    const count = (width: number) =>
      RING_ORDER.map((ring) => SLOTS[ring].filter((slot) => slot.minField <= width).length);
    expect([508, 888, 1228].map(count)).toEqual([
      [4, 3],
      [5, 8],
      [6, 12],
    ]);
  });
});
