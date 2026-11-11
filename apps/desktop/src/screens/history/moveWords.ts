import type { TFunction } from 'i18next';
import { isNearDay } from '../../i18n/format';
import type { Formatters } from '../../i18n/useFormat';
import type { IconName } from '../../ui';
import type { HistoryEntry, MoveKind } from './model';

/** The glyph for each kind of move. */
export const KIND_ICON: Record<MoveKind, IconName> = {
  received: 'receive',
  sent: 'send',
  saved: 'archive',
  opened: 'archive',
  undone: 'undo',
};

/** "Received from ezio-desktop", "Saved to a file", … */
export function moveTitle(entry: HistoryEntry, t: TFunction): string {
  return t(`history.kind.${entry.kind}`, { peer: entry.peer ?? t('history.anotherComputer') });
}

/** "Yesterday, 14:02. 5 apps, 1.2 GB": when, then what moved (when recorded). */
export function moveSub(entry: HistoryEntry, now: Date, t: TFunction, format: Formatters): string {
  const when = isNearDay(entry.when, now)
    ? t('summary.dayTime', {
        day: format.day(entry.when, now, true),
        time: format.time(entry.when),
      })
    : format.date(entry.when);
  const moved = entry.moved;
  if (moved === null) return when;
  const apps = t('count.apps', { count: moved.apps });
  const summary =
    moved.bytes === null ? apps : t('summary.appsSize', { apps, size: format.bytes(moved.bytes) });
  return t('history.when', { when, summary });
}

/** The exceptions a History row calls out; a normal move carries none. */
export type MoveTag = 'interrupted' | 'undone' | 'undo_expired';

/** Interrupted, undone, or finished but its undo point is gone; `null` for any other move. */
export function moveTag(entry: HistoryEntry): MoveTag | null {
  if (entry.kind === 'undone') return 'undone';
  if (entry.resumable) return 'interrupted';
  const imported = entry.kind === 'received' || entry.kind === 'opened';
  return imported && entry.state === 'done' && !entry.undoable ? 'undo_expired' : null;
}

/** The History header's views of the list, in order. */
export const MOVE_FILTERS = ['all', 'sent', 'received', 'files'] as const;
export type MoveFilter = (typeof MOVE_FILTERS)[number];

/** Which kinds of move each view keeps (an undone move was a received one). */
const FILTER_KINDS: Record<MoveFilter, readonly MoveKind[] | null> = {
  all: null,
  sent: ['sent'],
  received: ['received', 'undone'],
  files: ['saved', 'opened'],
};

/** The moves in view `filter` whose other side's name contains `query`. */
export function visibleMoves(
  entries: readonly HistoryEntry[],
  filter: MoveFilter,
  query: string,
): HistoryEntry[] {
  const kinds = FILTER_KINDS[filter];
  const needle = query.trim().toLocaleLowerCase();
  return entries.filter(
    (e) =>
      (kinds === null || kinds.includes(e.kind)) &&
      (e.peer ?? '').toLocaleLowerCase().includes(needle),
  );
}

/** Consecutive entries that share a label ("Today", "October 2026"), in order. */
export function groupMoves<T>(
  items: readonly T[],
  label: (item: T) => string,
): { readonly label: string; readonly items: T[] }[] {
  const groups: { label: string; items: T[] }[] = [];
  for (const item of items) {
    const key = label(item);
    const last = groups.at(-1);
    if (last?.label === key) last.items.push(item);
    else groups.push({ label: key, items: [item] });
  }
  return groups;
}
