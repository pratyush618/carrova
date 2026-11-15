import { describe, expect, it } from 'vitest';
import type { EngineEvent } from '@/engine/types';
import {
  appReducer,
  bridgeAction,
  initialAppState,
  LINK_SESSION,
  type AppState,
} from '@/app/state/appState';

const feed = (session: string, ...events: EngineEvent[]): AppState =>
  events.reduce<AppState>(
    (state, event) => appReducer(state, { type: 'engine', session, event }),
    initialAppState,
  );

describe('appReducer', () => {
  it('creates a session on its first event', () => {
    const state = feed('s1', {
      event: 'state_changed',
      session: 's1',
      from: 'planned',
      to: 'transferring',
    });
    expect(state.sessions['s1']?.state).toBe('transferring');
  });

  it('tracks byte progress per phase', () => {
    const state = feed('s1', { event: 'progress', phase: 'transfer', done: 40, total: 100 });
    expect(state.sessions['s1']).toMatchObject({ phase: 'transfer', done: 40, total: 100 });
  });

  it('pauses on low disk space and clears the pause on resume', () => {
    const reason = {
      kind: 'low_disk_space',
      fs: '/home',
      available: 1,
      needed: 400,
      disk: null,
    } as const;
    const paused = feed('s1', { event: 'transfer_paused', reason });
    expect(paused.sessions['s1']?.paused).toEqual(reason);
    const resumed = appReducer(paused, {
      type: 'engine',
      session: 's1',
      event: { event: 'transfer_resumed' },
    });
    expect(resumed.sessions['s1']?.paused).toBeNull();
  });

  it('distinguishes a link that is retrying from one that is lost', () => {
    expect(feed('s1', { event: 'peer_lost', retrying: true }).sessions['s1']?.link).toBe(
      'retrying',
    );
    expect(feed('s1', { event: 'peer_lost', retrying: false }).sessions['s1']?.link).toBe('lost');
    expect(
      feed('s1', { event: 'peer_connected', peer: '192.168.1.20:47822' }).sessions['s1']?.link,
    ).toBe('connected');
  });

  it('counts failed pairing attempts and rollback outcomes', () => {
    const state = feed(
      's1',
      { event: 'pairing_failed' },
      { event: 'pairing_failed' },
      { event: 'rollback_step', target: 'a', outcome: 'restored' },
      { event: 'rollback_step', target: 'b', outcome: 'moved_aside' },
    );
    expect(state.sessions['s1']?.pairingFailures).toBe(2);
    expect(state.sessions['s1']?.rollback).toEqual({
      restored: 1,
      removed: 0,
      moved_aside: 1,
      failed: 0,
    });
  });

  it('times a pairing code from its arrival and shows "checking" until the outcome', () => {
    const view = {
      code: '7-acid-lemon',
      fallback: null,
      renewed: false,
      expired: false,
      expires_in_secs: 300,
    };
    const shown = appReducer(
      initialAppState,
      bridgeAction({ kind: 'pairing_code', session: 's1', code: view }, 1_000),
    );
    expect(shown.sessions['s1']?.code).toEqual({ view, expiresAt: 301_000 });
    const engine = (state: AppState, event: EngineEvent) =>
      appReducer(state, { type: 'engine', session: 's1', event });
    const checking = engine(shown, { event: 'pairing_in_progress' });
    expect(checking.sessions['s1']?.checking).toBe(true);
    expect(engine(checking, { event: 'pairing_failed' }).sessions['s1']?.checking).toBe(false);
    const renewed = appReducer(
      checking,
      bridgeAction(
        { kind: 'pairing_code', session: 's1', code: { ...view, renewed: true } },
        2_000,
      ),
    );
    expect(renewed.sessions['s1']?.checking).toBe(false);
  });

  it('keeps captured items in engine order', () => {
    const state = feed(
      's1',
      { event: 'item_captured', item: 'git/config', files: 2, bytes: 4000 },
      { event: 'item_captured', item: 'vscode/settings', files: 388, bytes: 1_200_000_000 },
    );
    expect(state.sessions['s1']?.captured.map((c) => c.item)).toEqual([
      'git/config',
      'vscode/settings',
    ]);
  });

  it('forgets a dismissed session and leaves others alone', () => {
    const two = appReducer(feed('a', { event: 'transfer_resumed' }), {
      type: 'engine',
      session: 'b',
      event: { event: 'transfer_resumed' },
    });
    expect(Object.keys(appReducer(two, { type: 'dismissed', session: 'a' }).sessions)).toEqual([
      'b',
    ]);
  });
});

describe('appReducer peer facts', () => {
  it("keeps the receiver's inventory and plan summary for the sender", () => {
    const state = feed(
      's1',
      {
        event: 'inventory_received',
        os: 'linux',
        carrova_version: '0.1.0',
        apps: [],
        free_space: 112_000_000_000,
      },
      { event: 'plan_summary_received', items: [] },
    );
    expect(state.sessions['s1']?.peerInventory).toMatchObject({
      os: 'linux',
      free_space: 112_000_000_000,
    });
    expect(state.sessions['s1']?.peerPlan).toEqual({ items: [] });
  });

  it('folds linked-device events into the link state, never into a session', () => {
    const link = (event: EngineEvent) => (state: AppState) =>
      appReducer(state, { type: 'engine', session: LINK_SESSION, event });
    const steps = [
      link({ event: 'link_up', device: 'p8' }),
      link({
        event: 'ask_received',
        ask: 7,
        device: 'p8',
        kind: { kind: 'file', name: 'a.pdf', size: 9 },
      }),
      link({ event: 'file_progress', device: 'p8', id: 1, done: 4, total: 9 }),
      link({ event: 'file_saved', device: 'p8', id: 1, path: '/home/ana/Downloads/a.pdf' }),
      link({ event: 'clip_skipped', reason: 'sensitive' }),
    ];
    const state = steps.reduce((s, step) => step(s), initialAppState);
    expect(state.sessions).toEqual({});
    expect(state.link.changes).toBe(1);
    expect(state.link.asks).toEqual([
      { id: 7, device: 'p8', kind: { kind: 'file', name: 'a.pdf', size: 9 } },
    ]);
    expect(state.link.transfers).toEqual([
      {
        device: 'p8',
        id: 1,
        file_name: 'a.pdf',
        done: 9,
        total: 9,
        state: { kind: 'done' },
        dismissed: false,
      },
    ]);
    expect(state.link.clip?.reason).toBe('sensitive');

    const expired = link({ event: 'ask_expired', ask: 7 })(state);
    expect(expired.link.asks).toEqual([]);
    const answered = appReducer(state, { type: 'ask_answered', id: 7 });
    expect(answered.link.asks).toEqual([]);
  });

  it('keeps a transfer dismissed while it runs on, and drops it once finished', () => {
    const link = (state: AppState, event: EngineEvent) =>
      appReducer(state, { type: 'engine', session: LINK_SESSION, event });
    let state = link(initialAppState, {
      event: 'file_progress',
      device: 'p8',
      id: 1,
      done: 0,
      total: 9,
    });
    state = link(state, { event: 'file_progress', device: 'p8', id: 2, done: 2, total: 8 });
    state = appReducer(state, { type: 'transfer_dismissed', device: 'p8', id: 2 });
    state = link(state, { event: 'file_progress', device: 'p8', id: 2, done: 4, total: 8 });
    expect(state.link.transfers.find((t) => t.id === 2)).toMatchObject({
      done: 4,
      total: 8,
      dismissed: true,
    });
    // The newest one still shown is the other file, not the one dismissed.
    expect(state.link.transfers.filter((t) => !t.dismissed).map((t) => t.id)).toEqual([1]);

    state = link(state, { event: 'file_saved', device: 'p8', id: 2, path: '/d/b.pdf' });
    expect(state.link.transfers.map((t) => t.id)).toEqual([1]);
  });

  it('dismissing the newest finished transfer also lets the older finished ones go', () => {
    const link = (state: AppState, event: EngineEvent) =>
      appReducer(state, { type: 'engine', session: LINK_SESSION, event });
    let state = link(initialAppState, { event: 'file_saved', device: 'p8', id: 1, path: '/d/a' });
    state = link(state, { event: 'file_saved', device: 'p8', id: 2, path: '/d/b' });
    state = appReducer(state, { type: 'transfer_dismissed', device: 'p8', id: 2 });
    expect(state.link.transfers).toEqual([]);
  });
});
