import type { TFunction } from 'i18next';
import { isNearDay } from '../../i18n/format';
import type { Formatters } from '../../i18n/useFormat';
import type { IconName, Tone } from '../../ui';
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

/** A move's status chip. */
export interface MoveTag {
  readonly tone: Tone;
  readonly icon?: IconName;
  readonly label: string;
}

/** Undone, failed, interrupted, or how it finished; `null` for a move that never got going. */
export function moveTag(entry: HistoryEntry, t: TFunction): MoveTag | null {
  if (entry.kind === 'undone')
    return { tone: 'neutral', icon: 'undo', label: t('history.tag.undone') };
  if (entry.state === 'failed') return { tone: 'danger', label: t('history.tag.failed') };
  if (entry.resumable) return { tone: 'warning', label: t('history.interrupted') };
  if (entry.state !== 'done') return null;
  if (entry.kind === 'saved') return { tone: 'neutral', label: t('history.tag.saved') };
  if (entry.kind === 'sent')
    return { tone: 'success', icon: 'check', label: t('history.tag.sent') };
  return { tone: 'success', icon: 'check', label: t('history.tag.arrived') };
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
