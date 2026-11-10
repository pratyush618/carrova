import { GB, MB, NOW, SENDER, YESTERDAY } from '../shared/fixtures';
import type { HistoryEntry, HistoryModel, SessionModel } from './model';

const at = (h: number, m: number): Date => new Date(2026, 10, 7, h, m);

const RECEIVED: HistoryEntry = {
  id: 's-1107',
  kind: 'received',
  peer: SENDER,
  when: YESTERDAY,
  moved: { apps: 5, bytes: 1.2 * GB },
  undoable: true,
  resumable: false,
};

export const historyList: HistoryModel = {
  now: NOW,
  entries: [
    RECEIVED,
    {
      id: 's-1102',
      kind: 'saved',
      peer: null,
      when: new Date(2026, 10, 2, 9, 15),
      moved: { apps: 3, bytes: 40 * MB },
      undoable: false,
      resumable: false,
    },
    {
      id: 's-1028',
      kind: 'undone',
      peer: SENDER,
      when: new Date(2026, 9, 28, 18, 40),
      moved: null,
      undoable: false,
      resumable: false,
    },
  ],
};

export const historyEmpty: HistoryModel = { now: NOW, entries: [] };

export const session: SessionModel = {
  entry: RECEIVED,
  timeline: [
    { kind: 'started', at: at(14, 0) },
    { kind: 'state', at: at(14, 2), state: 'done' },
  ],
  log: ['Moved: ssh/config, git/config', 'Skipped: gpg/trust (the folder is locked)'],
};
