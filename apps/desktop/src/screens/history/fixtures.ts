import { GB, MB, NOW, SENDER } from '../shared/fixtures';
import type { HistoryEntry, HistoryModel, SessionModel } from './model';

const at = (d: number, h: number, m: number): Date => new Date(2026, 10, d, h, m);

const RECEIVED: HistoryEntry = {
  id: 's-1108',
  kind: 'received',
  state: 'done',
  peer: SENDER,
  when: at(8, 9, 32),
  moved: { apps: 5, bytes: 1.2 * GB },
  undoable: true,
  resumable: false,
};

const SAVED: HistoryEntry = {
  id: 's-1102',
  kind: 'saved',
  state: 'done',
  peer: 'carrova-2026-11-02.carrova',
  when: at(2, 9, 15),
  moved: { apps: 3, bytes: 40 * MB },
  undoable: false,
  resumable: false,
};

const UNDONE: HistoryEntry = {
  id: 's-1028',
  kind: 'undone',
  state: 'rolled_back',
  peer: 'lab-desktop',
  when: new Date(2026, 9, 28, 18, 40),
  moved: { apps: 2, bytes: null },
  undoable: false,
  resumable: false,
};

const LOG = ['Moved: ssh/config, git/config', 'Skipped: gpg/trust (the folder is locked)'];

const session = (entry: HistoryEntry, started: Date): SessionModel => ({
  undoCounts: entry.undoable ? { restored: 1, removed: 12, movedAside: 0, blockers: [] } : null,
  entry,
  timeline: [
    { kind: 'started', at: started },
    { kind: 'state', at: entry.when, state: entry.state },
  ],
  log: LOG,
});

const ENTRIES = [RECEIVED, SAVED, UNDONE];

/** The newest move selected: its steps, the undo point, Undo. */
export const historyTimeline: HistoryModel = {
  now: NOW,
  entries: ENTRIES,
  selected: session(RECEIVED, at(8, 9, 30)),
};

/** A rolled-back move selected: its steps rewound. */
export const historyUndone: HistoryModel = {
  now: NOW,
  entries: ENTRIES,
  selected: session(UNDONE, new Date(2026, 9, 28, 18, 31)),
};

export const historyEmpty: HistoryModel = { now: NOW, entries: [], selected: null };
