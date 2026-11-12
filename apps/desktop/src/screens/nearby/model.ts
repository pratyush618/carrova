import type { OsKind, Ring, VisibilityChoice } from '../../engine/types';

/** A change to visibility or the known devices that the engine can refuse. */
export type DeviceChange = 'visibility' | 'rename' | 'forget' | 'reset';

/** How the page shows the devices. */
export type NearbyView = 'map' | 'list';

/** A device remembered and here now, one seen for the first time, or a remembered one not here. */
export type DeviceKind = 'known' | 'new' | 'away';

/** One device on the Nearby page. */
export interface DeviceEntry {
  /** Stable key: the device id, or `new:<name>` for a new one. */
  readonly key: string;
  /** What device commands take; `null` for a new device. */
  readonly id: string | null;
  readonly name: string;
  readonly os: OsKind | null;
  readonly kind: DeviceKind;
  /** The map ring (the engine's bucket; away devices sit outside). */
  readonly ring: Ring;
  readonly fingerprint: string | null;
  readonly lastMove: Date | null;
  /** Moves made together (0 for a new device). */
  readonly moves: number;
  /** Away with a saved Wake-on-LAN hint. */
  readonly canWake: boolean;
}

/** What each kind of device offers (Wake only where a hint is saved). */
export type DeviceAction = 'sendWithCode' | 'wake' | 'forget';

// Phase 3 gives known devices a code-less "Send"; until then every device here pairs with a code.
export const DEVICE_ACTIONS: Readonly<Record<DeviceKind, readonly DeviceAction[]>> = {
  known: ['sendWithCode'],
  new: ['sendWithCode'],
  away: ['wake', 'forget'],
};

/** The actions `device` offers now. */
export const deviceActions = (device: DeviceEntry): readonly DeviceAction[] =>
  DEVICE_ACTIONS[device.kind].filter((action) => action !== 'wake' || device.canWake);

/** The visibility choices, in the order they are offered. */
export const VISIBILITY_CHOICES: readonly VisibilityChoice[] = ['no_one', 'known', 'everyone'];

/** Each choice's catalogue key (`no_one` would read as an i18next plural form). */
export const VISIBILITY_KEY = {
  no_one: 'nobody',
  known: 'known',
  everyone: 'everyone',
} as const satisfies Record<VisibilityChoice, string>;

/** The visibility in force. */
export interface VisibilityModel {
  readonly choice: VisibilityChoice;
  /** When "everyone" ends. */
  readonly until: Date | null;
}

/** The outcome of the last Wake. */
export interface WakeNote {
  readonly name: string;
  /** `no_hint`: no network address is saved for it. */
  readonly outcome: 'sent' | 'no_hint' | 'failed';
}

export interface NearbyModel {
  readonly view: NearbyView;
  readonly here: readonly DeviceEntry[];
  readonly away: readonly DeviceEntry[];
  /** The device whose card is open (map). */
  readonly selected: string | null;
  /** `null` until the engine answered. */
  readonly visibility: VisibilityModel | null;
  /** This computer's name, when the system tells it. */
  readonly me: string | null;
  readonly wake: WakeNote | null;
  /** The last change to the devices the engine refused. */
  readonly failed: DeviceChange | null;
  readonly now: Date;
}

export interface NearbyIntents {
  readonly onView: (view: NearbyView) => void;
  /** Open a device's card (`null` closes it). */
  readonly onSelect: (key: string | null) => void;
  readonly onVisibility: (choice: VisibilityChoice) => void;
  /** Open Settings → Nearby & devices. */
  readonly onSettings: () => void;
  /** Start a send that pairs with a code. */
  readonly onSendWithCode: () => void;
  readonly onWake: (device: DeviceEntry) => void;
  readonly onForget: (device: DeviceEntry) => void;
  readonly onDismissWake: () => void;
}
