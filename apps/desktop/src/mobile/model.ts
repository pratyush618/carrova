import type { FeatureRowView, FeatureState, LinkedDeviceView } from '../engine/types';
import { PHASE1_FEATURES, type Phase1Feature } from '../screens/devices/model';

/** "Choose what to share": one setting per Phase 1 feature. */
export type ShareChoices = Readonly<Record<Phase1Feature, FeatureState>>;

/** Each feature's group, and what a new phone link starts with (spec, Policy). */
const SHARE_DEFAULTS: Readonly<
  Record<Phase1Feature, { readonly group: FeatureRowView['group']; readonly state: FeatureState }>
> = {
  clipboard: { group: 'quiet', state: 'on' },
  ring: { group: 'quiet', state: 'on' },
  files: { group: 'ask', state: 'ask' },
  handoff: { group: 'ask', state: 'ask' },
};

/** The choices "Choose what to share" starts with: clipboard and ring on, the rest ask. */
export const shareDefaults = (): ShareChoices => ({
  clipboard: SHARE_DEFAULTS.clipboard.state,
  ring: SHARE_DEFAULTS.ring.state,
  files: SHARE_DEFAULTS.files.state,
  handoff: SHARE_DEFAULTS.handoff.state,
});

/** `feature`'s row on `device`, set to `state` on this phone (the other side's as it is). */
export function shareRow(
  device: LinkedDeviceView,
  feature: Phase1Feature,
  state: FeatureState,
): FeatureRowView {
  const row = device.features.find((f) => f.feature === feature);
  const { group } = SHARE_DEFAULTS[feature];
  return { feature, group, remote: null, effective: 'off', ...row, local: state };
}

/** The settings `choices` change on `device`, in the spec's order. */
export const shareChanges = (
  device: LinkedDeviceView,
  choices: ShareChoices,
): readonly (readonly [Phase1Feature, FeatureState])[] =>
  PHASE1_FEATURES.filter(
    (feature) => device.features.find((f) => f.feature === feature)?.local !== choices[feature],
  ).map((feature) => [feature, choices[feature]] as const);

/** The devices `feature` reaches now: linked, with it on or asking on both sides. */
export const readyFor = (
  devices: readonly LinkedDeviceView[],
  feature: Phase1Feature,
): readonly LinkedDeviceView[] =>
  devices.filter(
    (d) =>
      d.state === 'linked' &&
      d.features.some((f) => f.feature === feature && f.effective !== 'off'),
  );
