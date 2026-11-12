import { describe, expect, it } from 'vitest';
import type { Ring } from '@/engine/types';
import type { DeviceKind } from '@/screens/nearby/model';
import { place, type Placement } from '@/screens/nearby/place';
import { MIN_FIELD, RING_ORDER, SLOTS } from '@/screens/nearby/spots';

interface Device {
  key: string;
  ring: Ring;
  kind: DeviceKind;
}

const KINDS: readonly DeviceKind[] = ['known', 'new', 'away'];
const RANK: Readonly<Record<DeviceKind, number>> = { known: 0, new: 1, away: 2 };
const WIDTHS = [MIN_FIELD, 888, 1228];

/** `count` devices, a third on the inner ring, kinds mixed. */
const network = (count: number): Device[] =>
  Array.from({ length: count }, (_, i) => ({
    key: `d${String(i)}`,
    ring: i % 3 === 0 ? 'inner' : 'outer',
    kind: KINDS[Math.floor(i / 2) % 3] ?? 'known',
  }));

const open = (ring: Ring, width: number) =>
  SLOTS[ring].filter((slot) => slot.minField <= width).length;

const slotOf = (placed: readonly Placement[], key: string) => placed.find((p) => p.key === key);

describe('place', () => {
  it('puts every device on its ring in an open slot of its own, or in "more"', () => {
    for (const width of WIDTHS)
      for (let count = 0; count <= 30; count += 1) {
        const devices = network(count);
        const { placed, more } = place(devices, [], width, null);
        const label = `${String(count)} at ${String(width)}`;
        expect([...placed.map((p) => p.key), ...more.map((d) => d.key)].sort(), label).toEqual(
          devices.map((d) => d.key).sort(),
        );
        for (const ring of RING_ORDER) {
          const on = placed.filter((p) => p.ring === ring);
          expect(on.length, label).toBeLessThanOrEqual(open(ring, width));
          expect(new Set(on.map((p) => p.slot)).size, label).toBe(on.length);
          for (const p of on)
            expect(SLOTS[ring][p.slot]?.minField, label).toBeLessThanOrEqual(width);
        }
        for (const p of placed)
          expect(devices.find((d) => d.key === p.key)?.ring, label).toBe(p.ring);
      }
  });

  it('fills a ring before sending its devices to "more"', () => {
    for (const width of WIDTHS) {
      const devices = network(30);
      const { placed, more } = place(devices, [], width, null);
      for (const ring of RING_ORDER)
        if (more.some((d) => d.ring === ring))
          expect(placed.filter((p) => p.ring === ring)).toHaveLength(open(ring, width));
    }
  });

  it('keeps every other device in its slot when one arrives or leaves', () => {
    for (const width of WIDTHS)
      for (let count = 0; count < 30; count += 1) {
        const before = place(network(count), [], width, null).placed;
        const grown = place(network(count + 1), before, width, null).placed;
        for (const p of before) expect(slotOf(grown, p.key), `+1 of ${String(count)}`).toEqual(p);
        const gone = `d${String(Math.floor(count / 2))}`;
        const shrunk = place(
          network(count).filter((d) => d.key !== gone),
          before,
          width,
          null,
        ).placed;
        for (const p of before)
          if (p.key !== gone) expect(slotOf(shrunk, p.key), `-1 of ${String(count)}`).toEqual(p);
      }
  });

  it('always places the selected device, on its ring', () => {
    for (const width of WIDTHS)
      for (let count = 1; count <= 30; count += 1) {
        const devices = network(count);
        const before = place(devices, [], width, null).placed;
        for (const { key, ring } of devices) {
          const placed = place(devices, before, width, key).placed;
          expect(slotOf(placed, key)?.ring, `${key} of ${String(count)}`).toBe(ring);
        }
      }
  });

  it("gives the selected device its ring's last slot when the ring is full, leaving the rest", () => {
    const devices = network(30);
    const before = place(devices, [], MIN_FIELD, null).placed;
    const left = devices.find((d) => d.ring === 'inner' && slotOf(before, d.key) === undefined);
    expect(left).toBeDefined();
    if (left === undefined) return;
    const last = SLOTS.inner.findLastIndex((slot) => slot.minField <= MIN_FIELD);
    const { placed, more } = place(devices, before, MIN_FIELD, left.key);
    expect(slotOf(placed, left.key)).toEqual({ key: left.key, ring: 'inner', slot: last });
    const bumped = before.find((p) => p.ring === 'inner' && p.slot === last);
    expect(more.map((d) => d.key)).toContain(bumped?.key);
    for (const p of before) if (p !== bumped) expect(slotOf(placed, p.key)).toEqual(p);
  });

  it('moves a device that changed ring', () => {
    const devices = network(4);
    const before = place(devices, [], MIN_FIELD, null).placed;
    const moved = devices.map((d) => (d.key === 'd0' ? { ...d, ring: 'outer' as const } : d));
    const { placed } = place(moved, before, MIN_FIELD, null);
    expect(slotOf(placed, 'd0')?.ring).toBe('outer');
    for (const p of before) if (p.key !== 'd0') expect(slotOf(placed, p.key)).toEqual(p);
  });

  it('moves only the devices whose slots are too wide when the field narrows', () => {
    for (let count = 0; count <= 30; count += 1) {
      const devices = network(count);
      const wide = place(devices, [], 1228, null).placed;
      const narrow = place(devices, wide, MIN_FIELD, null).placed;
      for (const p of wide)
        if ((SLOTS[p.ring][p.slot]?.minField ?? Infinity) <= MIN_FIELD)
          expect(slotOf(narrow, p.key), String(count)).toEqual(p);
    }
  });

  it('drops away devices first, then new ones, then known ones', () => {
    for (const width of WIDTHS)
      for (let count = 0; count <= 30; count += 1) {
        const { placed, more } = place(network(count), [], width, null);
        for (const ring of RING_ORDER) {
          const kept = network(count).filter(
            (d) => d.ring === ring && slotOf(placed, d.key) !== undefined,
          );
          const dropped = more.filter((d) => d.ring === ring);
          for (const d of dropped)
            for (const k of kept)
              expect(RANK[k.kind], `${String(count)} ${ring}`).toBeLessThanOrEqual(RANK[d.kind]);
        }
      }
  });

  it('keeps a placed device over a newcomer that would rank above it', () => {
    const away = Array.from({ length: open('outer', MIN_FIELD) }, (_, i) => ({
      key: `a${String(i)}`,
      ring: 'outer' as const,
      kind: 'away' as const,
    }));
    const before = place(away, [], MIN_FIELD, null).placed;
    const known = { key: 'k', ring: 'outer' as const, kind: 'known' as const };
    const { placed, more } = place([...away, known], before, MIN_FIELD, null);
    expect(placed).toEqual(before);
    expect(more).toEqual([known]);
  });
});
