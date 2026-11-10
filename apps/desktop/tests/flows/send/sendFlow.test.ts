import { describe, expect, it } from 'vitest';
import { emptySession } from '@/app/state/session';
import { scan } from '@/ipc/fixtures/scan';
import { selectIssues } from '@/ipc/fixtures/select-issues';
import {
  initialSendFlow,
  selectionReady,
  sendFlowReducer,
  type SendFlowAction,
} from '@/flows/send/sendFlow';
import {
  foundModel,
  linkedModel,
  pairingModel,
  sendMove,
  sendStep,
} from '@/flows/send/sendProgressViews';

const run = (...actions: SendFlowAction[]) => actions.reduce(sendFlowReducer, initialSendFlow);
const scanned = run({ type: 'scan', action: { type: 'ok', value: scan } });
const error = (code: 'pairing_failed' | 'network') => ({ code, message: 'x', hint: null });

describe('send flow', () => {
  it('opens the session and takes the suggested selection from the scan', () => {
    expect(scanned.session).toBe(scan.session);
    expect(scanned.selection).toEqual(scan.selection);
    expect(foundModel(scanned).status).toBe('found');
    expect(foundModel(run({ type: 'scan', action: { type: 'start' } })).status).toBe('scanning');
  });

  it('holds Continue while the engine reports issues with the selection', () => {
    const withIssues = sendFlowReducer(scanned, { type: 'selected', result: selectIssues });
    expect(selectionReady(withIssues)).toBe(false);
    const model = foundModel(withIssues);
    expect(model.status === 'found' && model.issues).toEqual([
      'sensitive item needs explicit confirmation',
    ]);
  });

  it("takes each edit's result from the engine, and keeps it when an edit is refused", () => {
    const edited = sendFlowReducer(scanned, { type: 'selected', result: selectIssues });
    expect(edited.selection).toEqual(selectIssues.selection);
    const refused = sendFlowReducer(edited, {
      type: 'selectFailed',
      error: { code: 'invalid_selection', message: 'unknown item', hint: null },
    });
    expect(refused.selection).toEqual(selectIssues.selection);
    expect(selectionReady(refused)).toBe(false);
  });

  it('shows the code until the receiver connects, then live, then done', () => {
    const pairing = sendFlowReducer(scanned, { type: 'run', action: { type: 'start' } });
    const session = emptySession(scan.session);
    expect(sendStep(pairing, session)).toBe('code');
    expect(pairingModel(pairing, session).status).toBe('preparing');
    const view = {
      code: '7-acid-lemon',
      fallback: null,
      renewed: false,
      expired: false,
      expires_in_secs: 300,
    };
    const code = { view, expiresAt: 300_000 };
    expect(pairingModel(pairing, { ...session, code })).toEqual({
      status: 'waiting',
      code: ['7', 'acid', 'lemon'],
      address: null,
      expiresAt: 300_000,
    });
    const expired = { ...code, view: { ...view, expired: true } };
    expect(pairingModel(pairing, { ...session, code: expired }).status).toBe('expired');
    expect(pairingModel(pairing, { ...session, code, checking: true })).toEqual({
      status: 'checking',
    });
    expect(sendStep(pairing, { ...session, link: 'connected' })).toBe('live');
    const report = {
      checklist: { moved: [], skipped: [], apps_to_install: [], rollback_available: false },
      chunks: 1,
      bytes: 1,
      outcome: 'applied' as const,
    };
    expect(
      sendStep(
        sendFlowReducer(pairing, { type: 'run', action: { type: 'ok', value: report } }),
        session,
      ),
    ).toBe('done');
  });

  it('locks pairing after too many wrong codes and explains any other failure', () => {
    const pairing = sendFlowReducer(scanned, { type: 'run', action: { type: 'start' } });
    const fail = (code: 'pairing_failed' | 'network') =>
      sendFlowReducer(pairing, { type: 'run', action: { type: 'fail', error: error(code) } });
    const session = emptySession(scan.session);
    expect(pairingModel(fail('pairing_failed'), session)).toEqual({ status: 'locked' });
    expect(pairingModel(fail('network'), session)).toEqual({
      status: 'failed',
      error: error('network'),
    });
    expect(sendStep(fail('network'), { ...session, link: 'connected' })).toBe('code');
  });

  it("names the receiver and its OS once linked, and keeps the move's facts", () => {
    const here = { os: 'linux' as const, name: 'ana-laptop' };
    const session = {
      ...emptySession(scan.session),
      link: 'connected' as const,
      peer: '192.168.1.7:47822',
    };
    expect(linkedModel(scanned, session, here, 'Another computer').peer).toEqual({
      name: '192.168.1.7:47822',
      os: null,
    });
    const inventory = {
      os: 'mac_os' as const,
      carrova_version: '0.1.0',
      apps: [],
      free_space: null,
    };
    const linked = linkedModel(
      scanned,
      { ...session, peerName: 'ana-mbp', peerInventory: inventory },
      here,
      'Another computer',
    );
    expect(linked).toMatchObject({ here, peer: { name: 'ana-mbp', os: 'mac_os' }, replaced: 0 });
    expect(linked.facts).toEqual(sendMove(scanned, here).facts);
    expect(sendMove(initialSendFlow, here).facts).toBeNull();
  });

  it('goes back to the selection, keeping it', () => {
    const back = run(
      { type: 'scan', action: { type: 'ok', value: scan } },
      { type: 'run', action: { type: 'start' } },
      { type: 'back' },
    );
    expect(back.step).toBe('found');
    expect(back.selection).toEqual(scan.selection);
  });
});
