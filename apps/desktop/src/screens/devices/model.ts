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

/** The features Phase 1 shows (the reserved ones stay hidden), in the spec's order. */
export const PHASE1_FEATURES = ['clipboard', 'ring', 'files', 'handoff'] as const;

export type Phase1Feature = (typeof PHASE1_FEATURES)[number];

export const isPhase1 = (feature: Feature): feature is Phase1Feature =>
  PHASE1_FEATURES.some((f) => f === feature);

/** Each Phase 1 feature's glyph. */
export const FEATURE_ICON: Readonly<Record<Phase1Feature, IconName>> = {
  clipboard: 'copy',
  ring: 'bell',
  files: 'file',
  handoff: 'external',
};

export interface DeviceIntents {
  readonly onFeature: (feature: Feature, state: FeatureState) => void;
  readonly onRing: () => void;
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
  /** A device answered: the user checks its key before it is linked. */
  | { readonly state: 'found'; readonly peer: PairPeerView }
  | { readonly state: 'expired' | 'failed' }
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
