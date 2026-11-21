import type { DeviceEntry } from '../../screens/nearby/model';
import type { SendTargets } from '../../screens/send/model';

/** Phones never take a setup; a device whose system is unknown may be a computer. */
const computer = (device: DeviceEntry): boolean => device.os !== 'android';

/** Most recent move first; never moved together last. */
const byRecentMove = (a: DeviceEntry, b: DeviceEntry): number =>
  (b.lastMove?.getTime() ?? 0) - (a.lastMove?.getTime() ?? 0);

/**
 * Where a move can go, from the Nearby page's devices: trusted computers
 * here first (no code, most recent move first), then trusted ones away
 * (greyed), then new computers here, which need a code once.
 */
export function sendTargets(devices: {
  readonly here: readonly DeviceEntry[];
  readonly away: readonly DeviceEntry[];
}): SendTargets {
  const here = devices.here.filter(computer);
  return {
    trusted: here.filter((d) => d.kind !== 'new').sort(byRecentMove),
    away: devices.away.filter(computer),
    fresh: here.filter((d) => d.kind === 'new'),
  };
}
