import type { TFunction } from 'i18next';
import type { HistoryEntry } from './model';

/** "Received from ezio-desktop", "Saved to a file", … */
export function moveTitle(entry: HistoryEntry, t: TFunction): string {
  return t(`history.kind.${entry.kind}`, { peer: entry.peer ?? t('history.anotherComputer') });
}
