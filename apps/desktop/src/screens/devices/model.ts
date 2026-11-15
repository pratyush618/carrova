import type { Feature, FeatureState, OsKind, PairInviteView } from '../../engine/types';
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

/** The Add a phone dialog: making a code, waiting for a scan, and how it ended. */
export type AddPhoneModel =
  | { readonly state: 'starting' }
  | {
      readonly state: 'waiting';
      readonly invite: PairInviteView;
      /** When the code stops working (ms since epoch). */
      readonly expiresAt: number;
    }
  | { readonly state: 'expired' | 'failed' }
  | { readonly state: 'linked'; readonly name: string | null };

export interface AddPhoneIntents {
  readonly onCancel: () => void;
  /** A new code, after it expired or failed. */
  readonly onRetry: () => void;
  readonly onDone: () => void;
  /** Open the new device's page. */
  readonly onOpen: () => void;
}
