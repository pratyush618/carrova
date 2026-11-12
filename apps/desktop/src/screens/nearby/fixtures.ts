import { NOW } from '../shared/fixtures';
import type { DeviceEntry, NearbyModel } from './model';

const DAY = 86_400_000;
const ago = (days: number) => new Date(NOW.getTime() - days * DAY);

const ana: DeviceEntry = {
  key: 'a1',
  id: 'a1',
  name: "Ana's laptop",
  os: 'linux',
  kind: 'known',
  ring: 'inner',
  fingerprint: '7F3A·19C2',
  lastMove: ago(2),
  moves: 5,
  canWake: false,
};

/** The mockup's network: two known devices answering, a new one, two known ones away. */
export const nearbyMap: NearbyModel = {
  view: 'map',
  here: [
    ana,
    {
      ...ana,
      key: 'd2',
      id: 'd2',
      name: 'Desk PC',
      os: 'windows',
      fingerprint: 'B04E·88D1',
      lastMove: NOW,
      moves: 2,
    },
    {
      key: 'new:new-mbp',
      id: null,
      name: 'new-mbp',
      os: 'mac_os',
      kind: 'new',
      ring: 'outer',
      fingerprint: null,
      lastMove: null,
      moves: 0,
      canWake: false,
    },
  ],
  away: [
    {
      ...ana,
      key: 'o3',
      id: 'o3',
      name: 'Old ThinkPad',
      kind: 'away',
      ring: 'outer',
      fingerprint: '2C9F·4A07',
      lastMove: ago(21),
      moves: 3,
      canWake: true,
    },
    {
      ...ana,
      key: 'w4',
      id: 'w4',
      name: 'Work laptop',
      os: 'windows',
      kind: 'away',
      ring: 'outer',
      fingerprint: 'E51B·03C6',
      lastMove: ago(60),
      moves: 1,
    },
  ],
  selected: null,
  visibility: { choice: 'known', until: null },
  me: 'ezio-thinkpad',
  wake: null,
  now: NOW,
};

/** Ana's laptop selected: its card is open. */
export const nearbySelected: NearbyModel = { ...nearbyMap, selected: 'a1' };

export const nearbyList: NearbyModel = { ...nearbyMap, view: 'list' };

/** Nobody answered (yet). */
export const nearbyEmpty: NearbyModel = { ...nearbyMap, here: [], away: [] };

/** Woke a device with no saved address. */
export const nearbyNoHint: NearbyModel = {
  ...nearbyList,
  wake: { name: 'Work laptop', outcome: 'no_hint' },
};
