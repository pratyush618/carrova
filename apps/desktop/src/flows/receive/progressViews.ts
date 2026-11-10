import type { SessionProgress } from '../../app/state/session';
import type { PlanReview } from '../../engine/types';
import type { CodeEntryModel, ReceiveProgressModel } from '../../screens/receive/model';
import type { OpenFileModel } from '../../screens/file/model';
import { appOf } from '../shared/keys';
import { appRef } from '../shared/paths';
import { transferModel, type SizedApp, type TransferLive } from '../shared/transferView';
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
      return { status: 'connecting', here, peer, code: entry.value.split('-') };
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

/**
 * Transfer and apply progress, or the failure that was rolled back. A sender
 * that stayed unreachable ends the task too, but the session is kept: that
 * stays a stopped transfer, to resume.
 */
export function receiveProgressModel(
  state: ReceiveFlowState,
  progress: SessionProgress,
  review: PlanReview | null,
  peer: string,
  live: TransferLive = {},
): ReceiveProgressModel {
  if (state.apply.status === 'failed' && progress.link !== 'lost') {
    return { status: 'failed', error: state.apply.error };
  }
  // An in-place resume reopens like the entry did: its status and error are the resume's.
  const entry = state.origin === 'lan' ? state.code : state.file;
  return {
    status: 'transfer',
    transfer: transferModel('receive', progress, arrivingApps(review), peer, live),
    resuming: entry.status === 'connecting' || entry.status === 'opening',
    resumeError: entry.error,
  };
}
