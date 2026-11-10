import { describe, expect, it } from 'vitest';
import type { EngineEvent } from '@/engine/types';
import { appReducer, initialAppState, type AppState } from '@/app/state/appState';

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
    const reason = { kind: 'low_disk_space', fs: '/home', available: 1, needed: 400 } as const;
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
});
