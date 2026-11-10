import type { RollbackReport, SessionRecord, SessionState } from '../../engine/types';
import type { HistoryEntry, MoveKind, SessionModel } from '../../screens/history/model';
import type { HomeModel } from '../../screens/start/model';
import type { UndoResultModel } from '../../screens/undo/model';
import { appOf } from '../shared/keys';

const fromUnix = (seconds: number): Date => new Date(seconds * 1000);

/** The last path segment (the archive's file name). */
const baseName = (path: string): string => path.slice(path.lastIndexOf('/') + 1);

function kindOf(record: SessionRecord): MoveKind {
  if (record.state === 'rolled_back') return 'undone';
  const file = record.source?.kind === 'archive';
  if (record.role === 'export') return file ? 'saved' : 'sent';
  return file ? 'opened' : 'received';
}

/** States an interrupted session can be resumed from. */
const RESUMABLE: Readonly<Record<SessionRecord['role'], readonly SessionState[]>> = {
  import: ['planned', 'transferring', 'staged', 'applying'],
  export: ['transferring'],
};

/** One History row. */
export function historyEntry(record: SessionRecord): HistoryEntry {
  const source = record.source;
  const moved = record.checklist?.moved ?? [];
  return {
    id: record.id,
    kind: kindOf(record),
    peer: source?.kind === 'archive' ? baseName(source.path) : null,
    when: fromUnix(record.updated_at),
    moved:
      moved.length === 0
        ? null
        : { apps: new Set(moved.map((k) => appOf(k) ?? k)).size, bytes: null },
    undoable:
      record.role === 'import' &&
      record.state === 'done' &&
      record.checklist?.rollback_available === true,
    resumable: source !== null && RESUMABLE[record.role].includes(record.state),
  };
}

/** The move-details sheet: timeline, the engine's (redacted) words, and what's possible. */
export function sessionModel(record: SessionRecord): SessionModel {
  const skipped = record.checklist?.skipped ?? [];
  return {
    entry: historyEntry(record),
    timeline: [
      { kind: 'started', at: fromUnix(record.created_at) },
      { kind: 'state', at: fromUnix(record.updated_at), state: record.state },
    ],
    log: [
      ...(record.error === null ? [] : [record.error]),
      ...skipped.map((s) => `${s.what}: ${s.reason}`),
    ],
  };
}

/** Home's "last move" line: the newest finished send or receive. */
export function homeModel(
  records: readonly SessionRecord[],
  now: Date,
  unknownPeer: string,
): HomeModel {
  const last = records.find((r) => r.state === 'done' && r.checklist !== null);
  if (last === undefined) return { now, last: null };
  const entry = historyEntry(last);
  return {
    now,
    last: {
      direction: last.role === 'import' ? 'received' : 'sent',
      peer: entry.peer ?? unknownPeer,
      when: entry.when,
      apps: entry.moved?.apps ?? 0,
      undoable: entry.undoable,
    },
  };
}

/** The folder files were moved aside into: the common parent of every moved-aside path. */
function movedAsideDir(report: RollbackReport): string | null {
  const parents = report.quarantined.map((q) => q.moved_to.split('/').slice(0, -1));
  const [first] = parents;
  if (first === undefined) return null;
  const differs = first.findIndex((segment, i) => parents.some((p) => p[i] !== segment));
  return first.slice(0, differs === -1 ? first.length : differs).join('/') || '/';
}

/** Undone: counts, what couldn't be restored and why, where changed files went. */
export function undoResultModel(report: RollbackReport): UndoResultModel {
  return {
    restored: report.restored.length,
    removed: report.removed.length,
    movedAside: report.quarantined.length,
    failures: report.failed.map((f) => ({ what: f.target, reason: f.reason })),
    movedAsideDir: movedAsideDir(report),
  };
}
