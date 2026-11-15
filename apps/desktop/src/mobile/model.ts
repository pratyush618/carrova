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

/** What a link the app was opened with asks for. */
export type OpenedIntent =
  | { readonly kind: 'clipboard' }
  | { readonly kind: 'files'; readonly paths: readonly string[] }
  | { readonly kind: 'handoff'; readonly url: string }
  | { readonly kind: 'text'; readonly text: string };

/** How shared text arrives: the rest is the text, percent-encoded UTF-8. */
const SHARED_TEXT = 'data:text/plain,';

/**
 * What `link` asks for: send the clipboard (tile, notification), send a
 * shared file (its path), hand a web link off, send shared text as a clip;
 * `null` for anything else.
 */
export function openedIntent(link: string): OpenedIntent | null {
  if (link === 'carrova://send-clipboard') return { kind: 'clipboard' };
  if (link.startsWith('http://') || link.startsWith('https://'))
    return { kind: 'handoff', url: link };
  if (link.startsWith(SHARED_TEXT))
    return { kind: 'text', text: decodeURIComponent(link.slice(SHARED_TEXT.length)) };
  if (link.startsWith('file:///'))
    return { kind: 'files', paths: [decodeURIComponent(new URL(link).pathname)] };
  return null;
}

/** What `links` ask for, oldest first; one share's files become one send. */
export function openedIntents(links: readonly string[]): readonly OpenedIntent[] {
  const intents: OpenedIntent[] = [];
  for (const intent of links.map(openedIntent)) {
    const last = intents.at(-1);
    if (intent?.kind === 'files' && last?.kind === 'files')
      intents[intents.length - 1] = { kind: 'files', paths: [...last.paths, ...intent.paths] };
    else if (intent !== null) intents.push(intent);
  }
  return intents;
}
