import { describe, expect, it } from 'vitest';
import { appReducer, bridgeAction, initialAppState } from '@/app/state/appState';
import { applyRequests, noRequests, type RequestsAction } from '@/app/state/requests';
import type { MoveRequestView } from '@/engine/types';

const request = (id: number, device = 'd2', name = 'Desk PC'): MoveRequestView => ({
  id,
  device,
  name,
  summary: { apps: 12, bytes: 340_000_000, has_sensitive: true },
  expires_at: 0,
});
const run = (...actions: RequestsAction[]) => actions.reduce(applyRequests, noRequests);

describe('move requests waiting here', () => {
  it('keeps one per computer: a newer request replaces its older one', () => {
    const state = run(
      { type: 'request_arrived', request: request(1) },
      { type: 'request_arrived', request: request(2, 'a1', "Ana's laptop") },
      { type: 'request_arrived', request: request(3) },
    );
    expect(state.pending.map((r) => r.id)).toEqual([2, 3]);
  });

  it('says only that a request expired; other endings leave silently', () => {
    const arrived = run({ type: 'request_arrived', request: request(1) });
    const expired = applyRequests(arrived, { type: 'request_ended', id: 1, why: 'expired' });
    expect(expired).toEqual({ pending: [], ended: { name: 'Desk PC', why: 'expired' } });
    for (const why of ['cancelled', 'superseded', 'lost'] as const) {
      expect(applyRequests(arrived, { type: 'request_ended', id: 1, why })).toEqual(noRequests);
    }
    expect(applyRequests(expired, { type: 'ended_dismissed' }).ended).toBeNull();
  });

  it('drops an answered request at once', () => {
    const state = run(
      { type: 'request_arrived', request: request(1) },
      { type: 'request_answered', id: 1 },
    );
    expect(state.pending).toEqual([]);
  });

  it('folds the bridge events into app state', () => {
    const arrived = appReducer(
      initialAppState,
      bridgeAction({ kind: 'move_request_arrived', request: request(1) }, 0),
    );
    expect(arrived.requests.pending).toEqual([request(1)]);
    const ended = appReducer(
      arrived,
      bridgeAction({ kind: 'move_request_ended', id: 1, why: 'lost' }, 0),
    );
    expect(ended.requests).toEqual(noRequests);
  });
});
