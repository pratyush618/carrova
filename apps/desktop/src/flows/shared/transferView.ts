import type { SessionProgress } from '../../app/state/session';
import type { AppRef, Bytes } from '../../screens/shared/model';
import type { TransferApp, TransferModel, TransferPhase } from '../../screens/shared/transfer';

/** An app that moves, with the bytes it moves. */
export interface SizedApp extends AppRef {
  readonly bytes: Bytes;
}

/** Where the transfer is, from the session's events. A pause or a lost link wins. */
export function transferPhase(progress: SessionProgress): TransferPhase {
  if (progress.paused !== null) return progress.paused.kind === 'by_user' ? 'held' : 'paused';
  if (progress.link === 'retrying') return 'reconnecting';
  return progress.phase === 'apply' ? 'applying' : 'moving';
}

/**
 * Chip states from overall progress. The engine moves small apps first, so
 * apps sorted by size fill up in order: done, then the one moving, then waiting.
 */
export function chipStates(apps: readonly SizedApp[], fraction: number): TransferApp[] {
  const ordered = [...apps].sort((a, b) => a.bytes - b.bytes);
  const total = ordered.reduce((n, a) => n + a.bytes, 0);
  let moved = Math.min(1, Math.max(0, fraction)) * total;
  return ordered.map((app) => {
    const share = Math.min(app.bytes, moved);
    moved -= share;
    if (share >= app.bytes && app.bytes > 0) return { ...app, state: 'done', bytesDone: 0 };
    if (share > 0) return { ...app, state: 'moving', bytesDone: share };
    return { ...app, state: 'waiting', bytesDone: 0 };
  });
}

/** The progress screens' model for one session. */
export function transferModel(
  side: TransferModel['side'],
  progress: SessionProgress,
  apps: readonly SizedApp[],
  peer: string,
): TransferModel {
  const phase = transferPhase(progress);
  const fallbackTotal = apps.reduce((n, a) => n + a.bytes, 0);
  const total = progress.total > 0 ? progress.total : fallbackTotal;
  const done = progress.total > 0 ? progress.done : 0;
  const paused = progress.paused;
  return {
    side,
    phase,
    peer,
    done,
    total,
    secondsLeft: null,
    apps: chipStates(apps, total === 0 ? 0 : done / total),
    shortfall:
      paused?.kind === 'low_disk_space' ? Math.max(0, paused.needed - paused.available) : 0,
  };
}
