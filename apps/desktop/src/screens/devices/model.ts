import type {
  Feature,
  FeatureState,
  LinkedDeviceView,
  OsKind,
  PairInviteView,
  PairPeerView,
} from '../../engine/types';
import type { IconName } from '../../ui';

/** A phone or a computer, by its system. */
export const deviceIcon = (os: OsKind | null): IconName => (os === 'android' ? 'phone' : 'laptop');

/** A phone or a computer, as the copy names it. */
export type DeviceKind = 'phone' | 'computer';

export const deviceKind = (os: OsKind): DeviceKind => (os === 'android' ? 'phone' : 'computer');

/** The features Phase 2 shows (the reserved ones stay hidden), in the spec's order. */
export const PHASE2_FEATURES = [
  'clipboard',
  'ring',
  'files',
  'handoff',
  'notifications',
  'messages',
  'calls',
] as const;

export type Phase2Feature = (typeof PHASE2_FEATURES)[number];

export const isPhase2 = (feature: Feature): feature is Phase2Feature =>
  PHASE2_FEATURES.some((f) => f === feature);

/** Each Phase 2 feature's glyph. */
export const FEATURE_ICON: Readonly<Record<Phase2Feature, IconName>> = {
  clipboard: 'copy',
  ring: 'ring',
  files: 'file',
  handoff: 'external',
  notifications: 'bell',
  messages: 'msg',
  calls: 'phone',
};

export interface DeviceIntents {
  readonly onFeature: (feature: Feature, state: FeatureState) => void;
  readonly onRing: () => void;
  readonly onStopRing: () => void;
  readonly onSendClipboard: () => void;
  /** Absent where this build has no file picker. */
  readonly onSendFiles?: () => void;
  readonly onForget: () => void;
}

/** The Add a device dialog: making a code, waiting for a scan, the key check, and how it ended. */
export type AddDeviceModel =
  | { readonly state: 'starting' }
  | {
      readonly state: 'waiting';
      readonly invite: PairInviteView;
      /** When the code stops working (ms since epoch). */
      readonly expiresAt: number;
    }
  /** A device answered: the user checks its key before it is linked; `answered` once they chose. */
  | { readonly state: 'found'; readonly peer: PairPeerView; readonly answered: boolean }
  | { readonly state: 'expired' | 'failed' }
  /** The user said the device found wasn't theirs: nothing linked. */
  | { readonly state: 'rejected'; readonly kind: DeviceKind }
  /** `device`: the one that linked, when the list names it. */
  | { readonly state: 'linked'; readonly device: LinkedDeviceView | null };

export interface AddDeviceIntents {
  readonly onCancel: () => void;
  /** A new code, after it expired or failed. */
  readonly onRetry: () => void;
  readonly onDone: () => void;
  /** Open the new device's page. */
  readonly onOpen: () => void;
  /** The device found is the user's: link it. */
  readonly onConfirm: () => void;
  /** The device found is not the user's: link nothing. */
  readonly onNotMine: () => void;
}
