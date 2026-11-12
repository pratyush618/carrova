import type { Ring } from '../../engine/types';
import type { DeviceEntry, DeviceKind } from './model';
import { SLOTS } from './spots';

/** A device's slot on its ring (an index into `SLOTS[ring]`). */
export interface Placement {
  readonly key: string;
  readonly ring: Ring;
  readonly slot: number;
}

/** When the map is full, newcomers are left off in this order: away first, known last. */
const KEEP: Readonly<Record<DeviceKind, number>> = { known: 0, new: 1, away: 2 };

/**
 * Seats `devices` in the slots open on a `fieldWidth` field. A device keeps
 * its slot from `prev` while it stays on that ring and the slot stays open;
 * the others take their ring's first free slot, best kept first. The
 * selected device always shows, bumping its full ring's last slot. Whoever
 * finds no slot is in `more`, in input order.
 */
export function place<D extends Pick<DeviceEntry, 'key' | 'ring' | 'kind'>>(
  devices: readonly D[],
  prev: readonly Placement[],
  fieldWidth: number,
  selected: string | null,
): { placed: Placement[]; more: D[] } {
  const open = (ring: Ring, slot: number) =>
    (SLOTS[ring][slot]?.minField ?? Infinity) <= fieldWidth;
  const placed: Placement[] = [];
  const holder = (ring: Ring, slot: number) =>
    placed.findIndex((p) => p.ring === ring && p.slot === slot);
  const fresh: D[] = [];
  for (const device of devices) {
    const was = prev.find((p) => p.key === device.key);
    if (was?.ring === device.ring && open(was.ring, was.slot) && holder(was.ring, was.slot) < 0)
      placed.push(was);
    else fresh.push(device);
  }
  const first = (device: D) => (device.key === selected ? -1 : KEEP[device.kind]);
  for (const { key, ring } of fresh.sort((a, b) => first(a) - first(b))) {
    const free = SLOTS[ring].findIndex((_, slot) => open(ring, slot) && holder(ring, slot) < 0);
    if (free >= 0) placed.push({ key, ring, slot: free });
    else if (key === selected) {
      const last = SLOTS[ring].findLastIndex((_, slot) => open(ring, slot));
      if (last >= 0) placed.splice(holder(ring, last), 1, { key, ring, slot: last });
    }
  }
  const seated = new Set(placed.map((p) => p.key));
  return { placed, more: devices.filter((device) => !seated.has(device.key)) };
}
