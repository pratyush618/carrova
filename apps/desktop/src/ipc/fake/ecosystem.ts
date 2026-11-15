/**
 * The fake's linked devices: a pairing invite that a "Pixel 8" answers a
 * few steps later (linked once its key check is confirmed), per-device
 * features with the engine's two-sided gate, and the clipboard history.
 * Only the `linked` scenario starts with a device; every other one starts
 * with none.
 */
import type { FeatureRowView } from '../../bindings/FeatureRowView';
import type { FeatureState } from '../../bindings/FeatureState';
import type { ForgetDevices } from '../../bindings/ForgetDevices';
import type { LinkedDeviceView } from '../../bindings/LinkedDeviceView';
import type { OsKind } from '../../bindings/OsKind';
import type { PairInviteView } from '../../bindings/PairInviteView';
import type { PairPeerView } from '../../bindings/PairPeerView';
import type { ClipItemView } from '../../bindings/ClipItemView';
import type { ClipSettings } from '../../bindings/ClipSettings';
import type { CarrovaPort } from '../port';
import { bridgeError, raise } from '../reject';
import { CANCELLED, later, type Runtime } from './runtime';
import type { Scenario } from './scenario';

/** The bridge's session for linked-device events (`carrova_view::ipc::LINK_SESSION`). */
const LINK = 'link';

/** Pacing steps between the invite showing and the phone linking. */
const PAIR_STEPS = 10;

/** A placeholder QR (three finder squares); the real one comes from the engine. */
const QR_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 25 25"><rect width="25" height="25" fill="#fff"/><path d="M0 0h7v7H0zM18 0h7v7h-7zM0 18h7v7H0z"/></svg>';

const INVITE: PairInviteView = {
  uri: 'carrova://pair?v=1&t=4-maple-orbit@192.168.1.20:47100&k=FAKE',
  qr_svg: QR_SVG,
  code: {
    code: '4-maple-orbit',
    fallback: null,
    renewed: false,
    expired: false,
    expires_in_secs: 180,
  },
  expires_in_s: 180,
};

/** What the two-sided gate lets through (`carrova_link::policy`). */
function gate(local: FeatureState, remote: FeatureState | null): FeatureState {
  if (remote === null || local === 'off' || remote === 'off') return 'off';
  return local === 'ask' || remote === 'ask' ? 'ask' : 'on';
}

const row = (
  feature: FeatureRowView['feature'],
  local: FeatureState,
  remote: FeatureState | null = local,
): FeatureRowView => ({
  feature,
  group: feature === 'clipboard' || feature === 'ring' ? 'quiet' : 'ask',
  local,
  remote,
  effective: gate(local, remote),
});

/** A newly linked phone: the Phase 1 defaults on both sides. */
const phone = (handoffOnPhone: FeatureState = 'ask'): LinkedDeviceView => ({
  id: 'p8',
  name: 'Pixel 8',
  os: 'android',
  fingerprint: 'C4D2·7781',
  state: 'linked',
  features: [
    row('clipboard', 'on'),
    row('ring', 'on'),
    row('files', 'ask'),
    row('handoff', 'ask', handoffOnPhone),
  ],
});

/** A linked computer: everything starts off (`existing known computer`). */
const computer = (): LinkedDeviceView => ({
  id: 'd2',
  name: 'Desk PC',
  os: 'linux',
  fingerprint: 'B04E·88D1',
  state: 'linked',
  features: [row('clipboard', 'off'), row('files', 'off'), row('handoff', 'off')],
});

/** A computer as a phone sees it: the new phone link's defaults on both sides. */
const computerFromPhone = (): LinkedDeviceView => ({ ...computer(), features: phone().features });

const seedClips = (now: number): ClipItemView[] => [
  { kind: 'text', preview: 'Meet at 7, gate B', at: now - 120 },
  { kind: 'image', preview: '', at: now - 3_600 },
  { kind: 'text', preview: 'https://docs.rs/tokio/latest/tokio/', at: now - 10_800 },
];

type EcosystemPort = Pick<
  CarrovaPort,
  | 'linkedDevices'
  | 'pairInvite'
  | 'answerPairPeer'
  | 'linkComputer'
  | 'linkInvite'
  | 'ownFingerprint'
  | 'scanQr'
  | 'setFeature'
  | 'answerAsk'
  | 'sendClipboard'
  | 'sendFiles'
  | 'handoffUrl'
  | 'ring'
  | 'stopRing'
  | 'clipHistory'
  | 'clearClipHistory'
  | 'clipSettings'
  | 'setClipSettings'
  | 'retryFile'
>;

/** The fake's linked devices, plus what `cancelConnect` and `forgetDevice` do to them. */
export interface FakeEcosystem extends EcosystemPort {
  /** Stop a pairing still waiting (`cancelConnect`). */
  readonly cancel: () => void;
  /** Unlink forgotten devices (`forgetDevice`). */
  readonly forget: (which: ForgetDevices) => void;
}

/**
 * The linked-device commands of a fake port acting out `scenario` on a
 * device running `os`: a phone links only computers, with the phone's defaults.
 */
export function fakeEcosystem(runtime: Runtime, scenario: Scenario, os: OsKind): FakeEcosystem {
  const seeded = scenario === 'linked';
  const onPhone = os === 'android';
  const linkedComputer = onPhone ? computerFromPhone : computer;
  const seed = onPhone ? [linkedComputer()] : [phone('off'), computer()];
  let linked: LinkedDeviceView[] = seeded ? seed : [];
  let clips = seeded ? seedClips(Math.floor(Date.now() / 1000)) : [];
  let pairing: { cancelled: boolean } | null = null;
  /** Resolves the open key check (`pair_peer_found`); `null` when cancelled. */
  let peerAnswer: ((accept: boolean | null) => void) | null = null;
  let clipSettings: ClipSettings = { share: true, show_note: false, history_file: false };
  const find = (id: string) =>
    linked.find((d) => d.id === id) ??
    raise(bridgeError('known_device', 'no linked device matches'));
  const link = (device: LinkedDeviceView) => {
    linked = [...linked.filter((d) => d.id !== device.id), device];
    runtime.engine(LINK, { event: 'link_up', device: device.id });
  };
  /** Show `device` for the key check; resolves with the answer, `null` if cancelled. */
  const askPeer = (device: LinkedDeviceView) =>
    new Promise<boolean | null>((resolve) => {
      peerAnswer = resolve;
      const peer: PairPeerView = {
        name: device.name,
        os: device.os,
        fingerprint: device.fingerprint,
      };
      runtime.emit({ kind: 'pair_peer_found', peer });
    });
  /** Wait for `device`, asking the user about it first when `check` (the inviting side). */
  const awaitPairing = async (device: LinkedDeviceView, check: boolean) => {
    const mine = { cancelled: false };
    pairing = mine;
    for (let i = 0; i < PAIR_STEPS; i++) await runtime.pause();
    if (mine.cancelled) raise(CANCELLED);
    const confirmed = !check || (await askPeer(device));
    pairing = null;
    if (confirmed === null) raise(CANCELLED);
    if (!confirmed) raise(bridgeError('pairing_failed', 'the device found was not confirmed'));
    link(device);
  };
  return {
    linkedDevices: () => later(() => linked),
    pairInvite: async () => {
      await later(() => {
        runtime.emit({ kind: 'pair_invite', invite: INVITE });
      });
      await awaitPairing(phone(), true);
    },
    answerPairPeer: (accept) =>
      later(() => {
        const answer =
          peerAnswer ?? raise(bridgeError('session', 'no device is waiting to be confirmed'));
        peerAnswer = null;
        answer(accept);
      }),
    linkComputer: async (ticket) => {
      await later(() => {
        if (!/^\d+-[a-z]+-[a-z]+(@\S+)?$/.test(ticket.trim()))
          raise(bridgeError('bad_code', 'that is not a pairing code'));
      });
      await awaitPairing(linkedComputer(), false);
    },
    linkInvite: async (uri) => {
      await later(() => {
        if (!uri.startsWith('carrova://pair?'))
          raise(bridgeError('bad_code', 'that is not a Carrova pairing link'));
      });
      await awaitPairing(linkedComputer(), false);
    },
    ownFingerprint: () => later(() => 'A1B2·C3D4'),
    // The camera always finds the fake computer's code.
    scanQr: () => later(() => INVITE.uri),
    setFeature: (id, feature, state) =>
      later(() => {
        const device = find(id);
        const features = device.features.map((f) =>
          f.feature === feature ? row(feature, state, f.remote) : f,
        );
        linked = linked.map((d) => (d === device ? { ...d, features } : d));
        runtime.engine(LINK, { event: 'policy_changed', device: id });
      }),
    answerAsk: () => later(() => undefined),
    sendClipboard: (id) =>
      later(() => {
        runtime.engine(LINK, { event: 'clip_sent', device: find(id).id });
      }),
    sendFiles: (id) =>
      later(() => {
        find(id);
      }),
    handoffUrl: (id) =>
      later(() => {
        find(id);
      }),
    ring: (id) =>
      later(() => {
        runtime.engine(LINK, { event: 'ring_started', device: find(id).id });
      }),
    stopRing: (id) =>
      later(() => {
        runtime.engine(LINK, { event: 'ring_stopped', device: find(id).id });
      }),
    clipHistory: () => later(() => ({ items: clips })),
    clearClipHistory: () =>
      later(() => {
        clips = [];
      }),
    clipSettings: () => later(() => clipSettings),
    setClipSettings: (settings) =>
      later(() => {
        clipSettings = settings;
      }),
    // The fake never sends files, so no file waits to be sent again.
    retryFile: () =>
      later(() => raise(bridgeError('known_device', 'that file is no longer waiting'))),
    cancel: () => {
      if (pairing !== null) pairing.cancelled = true;
      peerAnswer?.(null);
      peerAnswer = null;
    },
    forget: (which) => {
      linked = which === 'all' ? [] : linked.filter((d) => d.id !== which.device);
    },
  };
}
