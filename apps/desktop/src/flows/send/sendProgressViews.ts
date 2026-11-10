import { peerLabel, type SessionProgress } from '../../app/state/session';
import type { SendReport } from '../../engine/types';
import type { FlowStep } from '../../app/nav/flows';
import type {
  DeliveredModel,
  FoundModel,
  LinkedModel,
  LiveModel,
  PairingModel,
  SendMove,
} from '../../screens/send/model';
import type { RouteEnd } from '../../ui';
import { current } from '../remote';
import type { SendFlowState } from './sendFlow';
import {
  activePreset,
  filterFound,
  foundApps,
  foundGroups,
  moveFacts,
  pluginAsks,
  secretItems,
  selectionIssues,
} from './sendViews';
import { transferModel, type SizedApp } from '../shared/transferView';
import { appOf } from '../shared/keys';

/** Which send screen shows: the code until the receiver connects, then live, then done. */
export function sendStep(state: SendFlowState, progress: SessionProgress): FlowStep<'send'> {
  switch (state.step) {
    case 'found':
      return 'found';
    case 'done':
      return 'done';
    case 'pairing':
      return state.run.status !== 'failed' && progress.link !== 'none' ? 'live' : 'code';
  }
}

/** "Here's what we found", from the scan and the user's selection, narrowed to `filter`. */
export function foundModel(state: SendFlowState, filter = ''): FoundModel {
  const scan = current(state.scan);
  if (scan === null) {
    return { status: 'scanning', foundSoFar: [], progress: null };
  }
  const groups = foundGroups(scan);
  const secrets = secretItems(scan);
  const unrecognised = scan.report.unrecognised.length;
  if (groups.length === 0 && secrets.length === 0 && unrecognised === 0) {
    return { status: 'nothing', knownApps: scan.report.apps.length };
  }
  return {
    status: 'found',
    ...filterFound(groups, secrets, filter),
    filter,
    preset: activePreset(scan.selection),
    unrecognised,
    facts: moveFacts(scan),
    plugins: pluginAsks(scan.view),
    issues: [
      ...selectionIssues(scan.view),
      ...(state.selectError === null ? [] : [state.selectError.message]),
    ],
  };
}

/** The move as the rail shows it: this computer and, once scanned, what moves. */
export function sendMove(state: SendFlowState, here: RouteEnd | null): SendMove {
  const scan = current(state.scan);
  return { here, facts: scan === null ? null : moveFacts(scan) };
}

/** The pairing screen. Too many wrong codes locks pairing; any other failure says why. */
export function pairingModel(state: SendFlowState, progress: SessionProgress): PairingModel {
  if (state.run.status === 'failed') {
    return state.run.error.code === 'pairing_failed'
      ? { status: 'locked' }
      : { status: 'failed', error: state.run.error };
  }
  if (progress.checking) return { status: 'checking' };
  if (progress.code === null) return { status: 'preparing' };
  const { view, expiresAt } = progress.code;
  return {
    status: view.renewed ? 'renewed' : view.expired ? 'expired' : 'waiting',
    code: view.code.split('-'),
    address: view.fallback,
    expiresAt,
  };
}

/** The apps being sent, with the bytes each moves (for progress chips). */
export function sendingApps(state: SendFlowState): SizedApp[] {
  const scan = current(state.scan);
  if (scan === null) return [];
  const picked = Object.entries(scan.selection.items).filter(([, c]) => c.included);
  return foundApps(scan)
    .map((app) => {
      const items = scan.report.apps.find((a) => a.id === app.id)?.items ?? [];
      const bytes = picked
        .filter(([key]) => appOf(key) === app.id)
        .reduce((n, [key]) => n + (items.find((i) => `${app.id}/${i.id}` === key)?.size ?? 0), 0);
      return { id: app.id, glyph: app.glyph, name: app.name, bytes };
    })
    .filter((app) => app.bytes > 0);
}

/** Connected: the receiver reviews (with how many of their files get replaced), then progress. */
export function liveModel(
  state: SendFlowState,
  progress: SessionProgress,
  here: string,
  unknownPeer: string,
): LiveModel {
  const peer = peerLabel(progress) ?? unknownPeer;
  const moving = progress.phase !== null && progress.phase !== 'capture';
  if (!moving) {
    const replaced = (progress.peerPlan?.items ?? []).reduce((n, i) => n + i.counts.replace, 0);
    return { status: 'reviewing', here, peer, replaced };
  }
  return {
    status: 'transfer',
    transfer: transferModel('send', progress, sendingApps(state), peer),
  };
}

/** Paired, while the receiver reviews: who they are and how many of their files get replaced. */
export function linkedModel(
  state: SendFlowState,
  progress: SessionProgress,
  here: RouteEnd | null,
  unknownPeer: string,
): LinkedModel {
  return {
    ...sendMove(state, here),
    peer: { name: peerLabel(progress) ?? unknownPeer, os: progress.peerInventory?.os ?? null },
    replaced: (progress.peerPlan?.items ?? []).reduce((n, i) => n + i.counts.replace, 0),
  };
}

/** Delivered: apps and bytes the receiver took. */
export function deliveredModel(report: SendReport, peer: string): DeliveredModel {
  const apps = new Set(report.checklist.moved.map((key) => appOf(key) ?? key));
  return { peer, apps: apps.size, bytes: report.bytes };
}
