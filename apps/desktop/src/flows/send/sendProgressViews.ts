import { peerLabel, type SessionProgress } from '../../app/state/session';
import type { SendReport } from '../../engine/types';
import type { FlowStep } from '../../app/nav/flows';
import type { DeliveredModel, FoundModel, LiveModel, PairingModel } from '../../screens/send/model';
import { current } from '../remote';
import type { SendFlowState } from './sendFlow';
import { foundApps, selectionIssues, sensitiveAsks } from './sendViews';
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

/** "Here's what we found", from the scan and the user's selection. */
export function foundModel(state: SendFlowState): FoundModel {
  const scan = current(state.scan);
  if (scan === null) {
    return { status: 'scanning', foundSoFar: [], progress: null };
  }
  const apps = foundApps(scan);
  if (apps.length === 0 && scan.report.unrecognised.length === 0) {
    return { status: 'nothing', knownApps: scan.report.apps.length };
  }
  return {
    status: 'found',
    apps,
    asks: sensitiveAsks(scan),
    issues: [
      ...selectionIssues(scan.view),
      ...(state.selectError === null ? [] : [state.selectError.message]),
    ],
  };
}

/** The pairing screen. Too many wrong codes locks pairing; any other failure says why. */
export function pairingModel(state: SendFlowState, progress: SessionProgress): PairingModel {
  if (state.run.status === 'failed') {
    return state.run.error.code === 'pairing_failed'
      ? { status: 'locked' }
      : { status: 'failed', error: state.run.error };
  }
  const code = progress.code;
  if (code === null) return { status: 'preparing' };
  return {
    status: code.renewed ? 'renewed' : 'waiting',
    code: code.code.split('-'),
    address: code.fallback,
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

/** Delivered: apps and bytes the receiver took. */
export function deliveredModel(report: SendReport, peer: string): DeliveredModel {
  const apps = new Set(report.checklist.moved.map((key) => appOf(key) ?? key));
  return { peer, apps: apps.size, bytes: report.bytes };
}
