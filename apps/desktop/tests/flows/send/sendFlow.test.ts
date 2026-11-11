import { describe, expect, it } from 'vitest';
import { emptySession } from '@/app/state/session';
import { scan } from '@/ipc/fixtures/scan';
import { selectIssues } from '@/ipc/fixtures/select-issues';
import { createFakePort } from '@/ipc/fake';
import {
  initialSendFlow,
  selectionReady,
  sendBusy,
  sendFlowReducer,
  type SendFlowAction,
} from '@/flows/send/sendFlow';
import {
  foundModel,
  linkedModel,
  modeSwitch,
  pairingModel,
  sendMove,
  sendStep,
} from '@/flows/send/sendProgressViews';
import { foundApps } from '@/flows/send/sendViews';

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

/** A Manual session from the fake: opened, then `paths` picked. */
async function manual(paths: string[]) {
  const port = createFakePort({ stepMs: 0 });
  const started = await port.startManual();
  const picked = await port.pick(started.session, started.selection, paths);
  return { started, picked };
}

describe('send flow: Manual mode', () => {
  const SITE = '/home/ana/projects/site';

  it('switching modes keeps both lists and shows the other count', async () => {
    const { picked } = await manual([SITE, '/home/ana/notes.txt']);
    const both = sendFlowReducer(sendFlowReducer(scanned, { type: 'mode', mode: 'manual' }), {
      type: 'selected',
      mode: 'manual',
      result: picked.scan,
    });
    const apps = foundApps(scan).filter((a) => a.included).length;
    expect(foundModel(both).status).toBe('manual');
    expect(both.selection).toEqual(picked.scan.selection);
    expect(modeSwitch(both)).toEqual({ manual: true, picks: 2, apps, disabled: false });

    const auto = sendFlowReducer(both, { type: 'mode', mode: 'auto' });
    expect(auto.selection).toEqual(scan.selection);
    expect(foundModel(auto).status).toBe('found');
    expect(modeSwitch(auto)).toEqual({ manual: false, picks: 2, apps, disabled: false });
    expect(sendFlowReducer(auto, { type: 'mode', mode: 'manual' }).selection).toEqual(
      picked.scan.selection,
    );
  });

  it("keeps an edit's result for its own mode when the other one shows", async () => {
    const { picked } = await manual([SITE]);
    const state = run(
      { type: 'scan', action: { type: 'ok', value: scan } },
      { type: 'mode', mode: 'manual' },
      { type: 'selected', mode: 'auto', result: selectIssues },
      { type: 'selected', mode: 'manual', result: picked.scan },
    );
    expect(state.selection).toEqual(picked.scan.selection);
    expect(sendFlowReducer(state, { type: 'mode', mode: 'auto' }).selection).toEqual(
      selectIssues.selection,
    );
  });

  it('pending_row_shows_while_engine_measures', async () => {
    const { started, picked } = await manual([SITE]);
    const measuring = run(
      { type: 'reset', mode: 'manual' },
      { type: 'scan', mode: 'manual', action: { type: 'ok', value: started } },
      { type: 'measuring', paths: [SITE] },
    );
    const model = foundModel(measuring);
    expect(model.status === 'manual' && model.measuring).toEqual(['~/projects/site']);
    expect(model.status === 'manual' && model.picks).toEqual([]);

    const done = sendFlowReducer(measuring, {
      type: 'picked',
      paths: [SITE],
      result: picked.scan,
      outcomes: picked.outcomes,
    });
    const after = foundModel(done);
    expect(after.status === 'manual' && after.measuring).toEqual([]);
    expect(after.status === 'manual' && after.picks.map((p) => p.path)).toEqual([
      '~/projects/site',
    ]);
  });

  it('rejected pick is listed with its reason kind', async () => {
    const { picked } = await manual([SITE, '/etc/hosts', `${SITE}/index.html`]);
    const state = run(
      { type: 'reset', mode: 'manual' },
      {
        type: 'picked',
        paths: [],
        result: picked.scan,
        outcomes: picked.outcomes,
      },
    );
    const model = foundModel(state);
    expect(model.status === 'manual' && model.rejected).toEqual([
      { path: '/etc/hosts', reason: { kind: 'outside_roots' } },
    ]);
    expect(model.status === 'manual' && model.absorbed).toEqual([
      { path: '~/projects/site/index.html', into: '~/projects/site' },
    ]);
  });

  it('waits for a pick, and a sensitive one counts only once brought', async () => {
    const { picked } = await manual(['/home/ana/.ssh/id_ed25519']);
    const state = run(
      { type: 'reset', mode: 'manual' },
      { type: 'picked', paths: [], result: picked.scan, outcomes: picked.outcomes },
    );
    const model = foundModel(state);
    expect(model.status === 'manual' && model.picks[0]).toMatchObject({
      sensitive: true,
      included: false,
    });
    expect(model.status === 'manual' && model.facts).toMatchObject({ picks: 0, keys: 0 });
  });

  it('is busy only while a send runs', () => {
    expect(sendBusy(scanned)).toBe(false);
    const sending = sendFlowReducer(scanned, { type: 'run', action: { type: 'start' } });
    expect(sendBusy(sending)).toBe(true);
    expect(
      sendBusy(
        sendFlowReducer(sending, {
          type: 'run',
          action: { type: 'fail', error: error('network') },
        }),
      ),
    ).toBe(false);
  });
});
