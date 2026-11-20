import { describe, expect, it } from 'vitest';
import { emptySession } from '@/app/state/session';
import type { ErrorCode } from '@/engine/types';
import { initialSendFlow, sendFlowReducer, type SendFlowAction } from '@/flows/send/sendFlow';
import { sendStep } from '@/flows/send/sendProgressViews';
import { REQUEST_MS, requestSummary, sendWaitModel } from '@/flows/send/sendWait';
import { scan } from '@/ipc/fixtures/scan';

const run = (...actions: SendFlowAction[]) => actions.reduce(sendFlowReducer, initialSendFlow);
const deskPc = { id: 'd2', name: 'Desk PC', at: 1_000 };
const asked = run(
  { type: 'scan', action: { type: 'ok', value: scan } },
  { type: 'request', to: deskPc },
  { type: 'run', action: { type: 'start' } },
);
const idle = emptySession(scan.session);
const connected = { ...idle, link: 'connected' as const };
const failed = (code: ErrorCode) =>
  sendFlowReducer(asked, {
    type: 'run',
    action: { type: 'fail', error: { code, message: 'x', hint: null } },
  });

describe('a requested send', () => {
  it('waits over the selection, never on a code screen', () => {
    expect(sendWaitModel(asked, idle)).toMatchObject({
      state: 'waiting',
      peer: 'Desk PC',
      expiresAt: 1_000 + REQUEST_MS,
    });
    expect(sendStep(asked, idle)).toBe('found');
  });

  it('shows the receiver reviewing once it connects, on the live step', () => {
    expect(sendWaitModel(asked, connected)?.state).toBe('reviewing');
    expect(sendStep(asked, connected)).toBe('live');
  });

  it('says how it ended, staying over the selection', () => {
    const endings: [ErrorCode, string][] = [
      ['request_declined', 'declined'],
      ['request_expired', 'timeout'],
      ['request_refused', 'needs_code'],
      ['network', 'away'],
      ['internal', 'away'],
    ];
    for (const [code, state] of endings) {
      expect(sendWaitModel(failed(code), idle)?.state, code).toBe(state);
      expect(sendStep(failed(code), idle), code).toBe('found');
    }
  });

  it('closes when cancelled, sent back, or paired with a code instead', () => {
    expect(sendWaitModel(failed('cancelled'), idle)).toBeNull();
    expect(sendWaitModel(sendFlowReducer(asked, { type: 'back' }), idle)).toBeNull();
    const byCode = sendFlowReducer(asked, { type: 'request', to: null });
    expect(sendWaitModel(byCode, idle)).toBeNull();
    expect(sendStep(byCode, idle)).toBe('code');
  });

  it('carries counts only: apps, bytes and whether keys go along', () => {
    const facts = { apps: 3, totalApps: 9, bytes: 1_200, keys: 0, caches: false };
    expect(requestSummary(facts)).toEqual({ apps: 3, bytes: 1_200, has_sensitive: false });
    expect(requestSummary({ ...facts, keys: 1 })?.has_sensitive).toBe(true);
    expect(requestSummary({ ...facts, secrets: 2 })?.has_sensitive).toBe(true);
    expect(requestSummary(null)).toBeNull();
  });
});
