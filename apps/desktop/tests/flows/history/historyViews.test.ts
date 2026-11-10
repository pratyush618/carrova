import { describe, expect, it } from 'vitest';
import type { SessionRecord } from '@/engine/types';
import {
  historyEntry,
  homeModel,
  sessionModel,
  undoResultModel,
} from '@/flows/history/historyViews';
import { rollbackPreview } from '@/ipc/fixtures/rollback-preview';

const record = (patch: Partial<SessionRecord>): SessionRecord => ({
  id: '20261107-140000-abcdef',
  role: 'import',
  state: 'done',
  created_at: 1_762_524_000,
  updated_at: 1_762_524_150,
  source: { kind: 'lan', peer_cert_hash: null, manifest_fingerprint: 'f' },
  approved: true,
  checklist: {
    moved: ['ssh/config', 'ssh/known-hosts', 'git/config'],
    skipped: [{ what: 'gpg/trust', reason: 'locked' }],
    apps_to_install: [],
    rollback_available: true,
  },
  error: null,
  ...patch,
});

describe('history views', () => {
  it('names each kind of move and counts apps, not items', () => {
    expect(historyEntry(record({}))).toMatchObject({
      kind: 'received',
      peer: null,
      moved: { apps: 2, bytes: null },
      undoable: true,
      resumable: false,
    });
    const saved = record({
      role: 'export',
      source: { kind: 'archive', path: '/home/u/setup.carrova' },
    });
    expect(historyEntry(saved)).toMatchObject({ kind: 'saved', peer: 'setup.carrova' });
    expect(historyEntry(record({ state: 'rolled_back' })).kind).toBe('undone');
  });

  it('offers resume for interrupted sessions only', () => {
    expect(historyEntry(record({ state: 'transferring', checklist: null })).resumable).toBe(true);
    expect(historyEntry(record({ state: 'failed' })).resumable).toBe(false);
    expect(historyEntry(record({ role: 'export', state: 'transferring' })).resumable).toBe(true);
  });

  it('shows the engine error and skipped items in the log', () => {
    const model = sessionModel(record({ error: 'Peer lost' }), null);
    expect(model.log).toEqual(['Peer lost', 'gpg/trust: locked']);
    expect(model.timeline.map((s) => s.kind)).toEqual(['started', 'state']);
    expect(model.undoCounts).toBeNull();
  });

  it("carries the engine's undo preview as counts for the rail", () => {
    const counts = sessionModel(record({}), rollbackPreview).undoCounts;
    expect(counts).toMatchObject({
      removed: rollbackPreview.removed,
      restored: rollbackPreview.restored,
    });
  });

  it('names a LAN peer by the device name it announced', () => {
    const lan = record({
      source: { kind: 'lan', manifest_fingerprint: 'f', peer_name: 'ezio-desktop' },
    });
    expect(historyEntry(lan).peer).toBe('ezio-desktop');
  });

  it('lists the three newest moves on Home and asks for consent only while unanswered', () => {
    const now = new Date();
    const records = ['a', 'b', 'c', 'd'].map((id) => record({ id }));
    const home = homeModel(records, now, 'unset');
    expect(home.recent.map((e) => e.id)).toEqual(['a', 'b', 'c']);
    expect(home.askReports).toBe(true);
    expect(homeModel([], now, 'denied')).toEqual({ now, recent: [], askReports: false });
    expect(homeModel([], now, null).askReports).toBe(false);
  });

  it('summarises a rollback with the folder changed files went to', () => {
    const model = undoResultModel({
      restored: ['/h/.bashrc'],
      removed: ['/h/a', '/h/b'],
      failed: [{ target: '/h/.zshrc', reason: 'busy' }],
      quarantined: [
        { target: '/h/x', moved_to: '/s/trash/one/x' },
        { target: '/h/y', moved_to: '/s/trash/two/y' },
      ],
    });
    expect(model).toEqual({
      restored: 1,
      removed: 2,
      movedAside: 2,
      failures: [{ what: '/h/.zshrc', reason: 'busy' }],
      movedAsideDir: '/s/trash',
    });
  });
});
