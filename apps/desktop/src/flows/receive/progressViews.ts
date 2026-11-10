import type { SessionProgress } from '../../app/state/session';
import type { PlanReview } from '../../engine/types';
import type { CodeEntryModel, ReceiveProgressModel } from '../../screens/receive/model';
import type { OpenFileModel } from '../../screens/file/model';
import { appOf } from '../shared/keys';
import { appRef } from '../shared/paths';
import { transferModel, type SizedApp } from '../shared/transferView';
import type { CodeEntry, FileEntry } from './entry';
import type { ReceiveFlowState } from './receiveFlow';

/** The code screen: typing (with word autocomplete), the handshake, or why it failed. */
export function codeEntryModel(
  entry: CodeEntry,
  words: readonly string[],
  here: string,
  peer: string,
): CodeEntryModel {
  switch (entry.status) {
    case 'connecting':
      return { status: 'connecting', here, peer };
    case 'failed':
      return {
        status: 'failed',
        value: entry.value,
        words,
        error: entry.error ?? { code: 'internal', message: '', hint: null },
      };
    case 'typing':
    case 'not-found':
    case 'mismatch':
      return {
        status: entry.status,
        value: entry.value,
        peer: null,
        words,
        address: entry.address,
      };
  }
}

export function openFileModel(entry: FileEntry, canChoose: boolean): OpenFileModel {
  return {
    path: entry.path,
    canChoose,
    passphrase: entry.passphrase,
    wrong: entry.status === 'wrong',
    opening: entry.status === 'opening',
    error: entry.status === 'failed' ? entry.error : null,
  };
}

/** Apps arriving, with the bytes each writes (for progress chips). */
export function arrivingApps(review: PlanReview | null): SizedApp[] {
  if (review === null) return [];
  const bytes = new Map<string, number>();
  for (const a of review.plan.actions) {
    const app = appOf(a.item) ?? a.item;
    bytes.set(app, (bytes.get(app) ?? 0) + a.size);
  }
  return [...bytes].map(([id, size]) => ({ ...appRef(id, id), bytes: size }));
}

/** Transfer and apply progress, or the failure that was rolled back. */
export function receiveProgressModel(
  state: ReceiveFlowState,
  progress: SessionProgress,
  review: PlanReview | null,
  peer: string,
): ReceiveProgressModel {
  if (state.apply.status === 'failed') return { status: 'failed', error: state.apply.error };
  return {
    status: 'transfer',
    transfer: transferModel('receive', progress, arrivingApps(review), peer),
  };
}
