import type { HistoryEntry } from '../history/model';

/** Home: first run teaches the task; returning users get their recent moves. */
export interface HomeModel {
  readonly now: Date;
  /** Newest first; empty on a fresh install (first run). */
  readonly recent: readonly HistoryEntry[];
  /** Error-report consent was never answered: ask once, off until answered. */
  readonly askReports: boolean;
}

export interface HomeIntents {
  readonly onSend: () => void;
  readonly onReceive: () => void;
  readonly onUseFile: () => void;
  /** Undo one of the recent moves (asks first). */
  readonly onUndo: (id: string) => void;
  readonly onHistory: () => void;
  readonly onErrorReports: (on: boolean) => void;
}

export interface FileChooseIntents {
  readonly onSave: () => void;
  readonly onOpen: () => void;
  readonly onCancel: () => void;
}
