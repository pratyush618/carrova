import type { SessionProgress } from '../../app/state/session';
import type { AppRef, Bytes, MoveRoute } from '../../screens/shared/model';
import type { TransferApp, TransferModel, TransferPhase } from '../../screens/shared/transfer';

/** An app that moves, with the bytes it moves. */
export interface SizedApp extends AppRef {
  readonly bytes: Bytes;
}

/** Where the transfer is, from the session's events. A pause or a lost link wins. */
export function transferPhase(progress: SessionProgress): TransferPhase {
  if (progress.paused !== null) return progress.paused.kind === 'by_user' ? 'held' : 'paused';
  if (progress.link === 'retrying') return 'reconnecting';
  if (progress.link === 'lost') return 'lost';
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

/** One reading of the bytes done so far, at `at` (ms). */
export interface RateSample {
  readonly at: number;
  readonly done: Bytes;
}

/** How far back the rate looks: long enough to smooth bursts, short enough to follow changes. */
const RATE_WINDOW_MS = 5_000;

/** `samples` with `next` added and readings older than the window dropped; a step back restarts. */
export function addSample(samples: readonly RateSample[], next: RateSample): RateSample[] {
  const last = samples.at(-1);
  if (last !== undefined && next.done < last.done) return [next];
  return [...samples.filter((s) => next.at - s.at <= RATE_WINDOW_MS), next];
}

/** Bytes per second over the window; `null` until it spans a second with progress. */
export function rateOf(samples: readonly RateSample[]): number | null {
  const first = samples[0];
  const last = samples.at(-1);
  if (first === undefined || last === undefined) return null;
  const ms = last.at - first.at;
  const bytes = last.done - first.done;
  return ms < 1_000 || bytes <= 0 ? null : (bytes * 1_000) / ms;
}

/** What the screens know beyond the session's events: the two computers and the measured rate. */
export interface TransferLive {
  readonly route?: MoveRoute | null;
  /** Bytes per second (`rateOf`), measured while bytes flow. */
  readonly rate?: number | null;
}

/** The progress screens' model for one session. */
export function transferModel(
  side: TransferModel['side'],
  progress: SessionProgress,
  apps: readonly SizedApp[],
  peer: string,
  live: TransferLive = {},
): TransferModel {
  const phase = transferPhase(progress);
  const fallbackTotal = apps.reduce((n, a) => n + a.bytes, 0);
  const total = progress.total > 0 ? progress.total : fallbackTotal;
  const done = progress.total > 0 ? progress.done : 0;
  const paused = progress.paused;
  const rate = phase === 'moving' ? (live.rate ?? null) : null;
  return {
    side,
    phase,
    peer,
    done,
    total,
    secondsLeft: rate === null ? null : Math.max(0, total - done) / rate,
    bytesPerSecond: rate,
    route: live.route ?? null,
    apps: chipStates(apps, total === 0 ? 0 : done / total),
    shortfall:
      paused?.kind === 'low_disk_space' ? Math.max(0, paused.needed - paused.available) : 0,
    disk: paused?.kind === 'low_disk_space' ? paused.disk : null,
  };
}
