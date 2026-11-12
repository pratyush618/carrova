/**
 * The fake's nearby devices, remembered devices and visibility. Only the
 * `nearby` scenario has devices; every other scenario starts with none.
 */
import type { ForgetDevices } from '../../bindings/ForgetDevices';
import type { KnownDeviceView } from '../../bindings/KnownDeviceView';
import type { NearbyDeviceView } from '../../bindings/NearbyDeviceView';
import type { VisibilityChoice } from '../../bindings/VisibilityChoice';
import type { VisibilityView } from '../../bindings/VisibilityView';
import type { CarrovaPort } from '../port';
import { bridgeError, raise } from '../reject';
import { later } from './runtime';
import type { Scenario } from './scenario';

const DAY = 86_400;

/** Remembered devices: two here now, two away (one with a wake hint). */
function seedKnown(now: number): KnownDeviceView[] {
  const device = (
    id: string,
    fingerprint: string,
    name: string,
    os: KnownDeviceView['os'],
    lastMove: number,
    moves: number,
    hint: boolean,
  ): KnownDeviceView => ({
    id,
    fingerprint,
    name,
    os,
    first_seen: now - 90 * DAY,
    last_move: now - lastMove,
    moves,
    has_wake_hint: hint,
  });
  return [
    device('a1', '7F3A·19C2', "Ana's laptop", 'linux', 2 * DAY, 5, false),
    device('d2', 'B04E·88D1', 'Desk PC', 'windows', 3600, 2, false),
    device('o3', '2C9F·4A07', 'Old ThinkPad', 'linux', 21 * DAY, 3, true),
    device('w4', 'E51B·03C6', 'Work laptop', 'windows', 60 * DAY, 1, false),
  ];
}

/** Who answered the browse: the two known devices here, and a new one offering a code. */
const HERE: readonly NearbyDeviceView[] = [
  {
    name: "Ana's laptop",
    os: 'linux',
    known: true,
    id: 'a1',
    fingerprint: '7F3A·19C2',
    answered_ms: 40,
    ring: 'inner',
    offers_code: false,
  },
  {
    name: 'Desk PC',
    os: 'windows',
    known: true,
    id: 'd2',
    fingerprint: 'B04E·88D1',
    answered_ms: 120,
    ring: 'inner',
    offers_code: false,
  },
  {
    name: 'new-mbp',
    os: 'mac_os',
    known: false,
    id: null,
    fingerprint: null,
    answered_ms: 900,
    ring: 'outer',
    offers_code: true,
  },
];

/** A device nobody has met before, offering a code. */
const stranger = (
  name: string,
  os: NearbyDeviceView['os'],
  ring: NearbyDeviceView['ring'],
  answered_ms: number,
): NearbyDeviceView => ({
  name,
  os,
  known: false,
  id: null,
  fingerprint: null,
  answered_ms,
  ring,
  offers_code: true,
});

/** The crowded split: one new device answers fast, four more slowly (names as long as real ones). */
const CROWDED: readonly NearbyDeviceView[] = [
  stranger("Ana's laptop", 'linux', 'inner', 60),
  stranger('new-mbp', 'mac_os', 'outer', 700),
  stranger('Studio Mac mini', 'mac_os', 'outer', 800),
  stranger('Living-room NUC', 'linux', 'outer', 900),
  stranger('Work ThinkPad X1', 'windows', 'outer', 1000),
];

/** "Everyone" lasts this long (the engine's `EVERYONE_FOR`). */
const EVERYONE_FOR = 600;

type DevicesPort = Pick<
  CarrovaPort,
  | 'nearby'
  | 'knownDevices'
  | 'renameDevice'
  | 'forgetDevice'
  | 'resetIdentity'
  | 'visibility'
  | 'setVisibility'
  | 'wakeDevice'
  | 'takeShowNearby'
>;

/** The device commands of a fake port acting out `scenario`. */
export function fakeDevices(scenario: Scenario): DevicesPort {
  const seeded = scenario === 'nearby' || scenario === 'nearby-crowded';
  const here = scenario === 'nearby-crowded' ? CROWDED : HERE;
  let known = scenario === 'nearby' ? seedKnown(Math.floor(Date.now() / 1000)) : [];
  let visibility: VisibilityView = { choice: 'known', until: null };
  const unknown = () => raise(bridgeError('known_device', 'no known device matches'));
  const find = (device: string) =>
    known.find((d) => d.id === device || d.fingerprint === device) ?? unknown();
  const kept = (which: ForgetDevices) => (d: KnownDeviceView) =>
    which !== 'all' && d.id !== which.device && d.fingerprint !== which.device;
  return {
    // A forgotten device that is still here answers as a new one.
    nearby: () =>
      later(() =>
        seeded
          ? here.map((d) =>
              d.id === null || known.some((k) => k.id === d.id)
                ? d
                : { ...d, known: false, id: null, fingerprint: null },
            )
          : [],
      ),
    knownDevices: () => later(() => known),
    renameDevice: (device, name) =>
      later(() => {
        const target = find(device);
        if (name.trim() === '') raise(bridgeError('known_device', 'a name cannot be empty'));
        known = known.map((d) => (d === target ? { ...d, name: name.trim() } : d));
      }),
    forgetDevice: (which) =>
      later(() => {
        known = known.filter(kept(which));
      }),
    resetIdentity: () =>
      later(() => {
        known = [];
      }),
    // "Everyone" runs out by itself, as in the engine.
    visibility: () =>
      later(() => {
        if (visibility.until !== null && visibility.until * 1000 <= Date.now())
          visibility = { choice: 'known', until: null };
        return visibility;
      }),
    setVisibility: (choice: VisibilityChoice) =>
      later(() => {
        visibility = {
          choice,
          until: choice === 'everyone' ? Math.floor(Date.now() / 1000) + EVERYONE_FOR : null,
        };
        return visibility;
      }),
    wakeDevice: (device) =>
      later(() => {
        if (!find(device).has_wake_hint)
          raise(bridgeError('known_device', 'no network address is saved for this device'));
      }),
    // The fake has no tray.
    takeShowNearby: () => later(() => false),
  };
}
