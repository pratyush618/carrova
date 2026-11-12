import { toBridgeError } from '../../engine/errors';
import type { KnownDeviceView, NearbyDeviceView, VisibilityView } from '../../engine/types';
import type { DeviceEntry, VisibilityModel, WakeNote } from '../../screens/nearby/model';
import type { KnownDeviceLine } from '../../screens/settings/model';

const date = (seconds: number | null): Date | null =>
  seconds === null ? null : new Date(seconds * 1000);

/** A device that answered the browse, with what this computer remembers of it. */
function hereEntry(
  device: NearbyDeviceView,
  record: KnownDeviceView | undefined,
  key: string,
): DeviceEntry {
  const known = device.known && device.id !== null;
  return {
    key,
    id: known ? device.id : null,
    name: device.name,
    os: device.os,
    kind: known ? 'known' : 'new',
    ring: device.ring,
    fingerprint: device.fingerprint,
    lastMove: date(record?.last_move ?? null),
    moves: record?.moves ?? 0,
    canWake: false,
  };
}

/** A remembered device that did not answer. */
function awayEntry(record: KnownDeviceView): DeviceEntry {
  return {
    key: record.id,
    id: record.id,
    name: record.name,
    os: record.os,
    kind: 'away',
    ring: 'outer',
    fingerprint: record.fingerprint,
    lastMove: date(record.last_move),
    moves: record.moves,
    canWake: record.has_wake_hint,
  };
}

/**
 * The Nearby page's devices: those that answered (in answer order), then the
 * remembered ones that did not.
 */
export function nearbyEntries(
  found: readonly NearbyDeviceView[],
  known: readonly KnownDeviceView[],
): { readonly here: DeviceEntry[]; readonly away: DeviceEntry[] } {
  const byId = new Map(known.map((record) => [record.id, record]));
  const answered = new Set(found.flatMap((device) => (device.id === null ? [] : [device.id])));
  // New devices have no id: key them by name, the 2nd of a name `new:<name>:2`, and so
  // on, skipping any key already taken (a device may be named `<name>:2`).
  const taken = new Set<string>();
  const newKey = (name: string): string => {
    let key = `new:${name}`;
    for (let nth = 2; taken.has(key); nth++) key = `new:${name}:${String(nth)}`;
    taken.add(key);
    return key;
  };
  return {
    here: found.map((device) =>
      device.id === null
        ? hereEntry(device, undefined, newKey(device.name))
        : hereEntry(device, byId.get(device.id), device.id),
    ),
    away: known.filter((record) => !answered.has(record.id)).map(awayEntry),
  };
}

/** The visibility in force, with dates. */
export const visibilityModel = (view: VisibilityView): VisibilityModel => ({
  choice: view.choice,
  until: date(view.until),
});

/** One row of Settings → Known devices. */
export const knownDeviceLine = (record: KnownDeviceView): KnownDeviceLine => ({
  id: record.id,
  name: record.name,
  os: record.os,
  fingerprint: record.fingerprint,
  lastMove: date(record.last_move),
});

/** Why a Wake failed: `known_device` means no network address is saved for it. */
export const wakeFailure = (reason: unknown): WakeNote['outcome'] =>
  toBridgeError(reason).code === 'known_device' ? 'no_hint' : 'failed';
