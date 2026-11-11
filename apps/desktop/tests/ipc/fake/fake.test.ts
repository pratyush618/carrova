import { describe, expect, it } from 'vitest';

import type { BridgeEvent } from '@/bindings/BridgeEvent';
import { ENGINE_DEFAULTS } from '@/bindings/constants';
import type { PlanOptions } from '@/bindings/PlanOptions';
import type { CarrovaPort } from '@/ipc/port';
import { createFakePort, scenarioFromSearch, WRONG_PASSPHRASE, type Scenario } from '@/ipc/fake';
import { plan } from '@/ipc/fixtures/plan';

const OPTIONS: PlanOptions = plan.plan.options;
/** Strong enough for the fake's meter (10+ characters). */
const PASS = 'correct horse battery staple';

/** A fake port with instant steps and the events it emits. */
async function fake(scenario: Scenario = 'happy') {
  const port: CarrovaPort = createFakePort({ scenario, stepMs: 0 });
  const events: BridgeEvent[] = [];
  await port.subscribe((event) => events.push(event));
  return { port, events };
}

/** Engine event names in `events`. */
function engineEvents(events: BridgeEvent[]): string[] {
  return events.flatMap((e) => (e.kind === 'engine' ? [e.event.event] : []));
}

/** Resolve `promise`'s rejection value. */
async function rejection(promise: Promise<unknown>): Promise<unknown> {
  try {
    await promise;
  } catch (reason: unknown) {
    return reason;
  }
  throw new Error('expected a rejection');
}

/** Until `predicate` holds for the events (the fake runs on microtasks/timers). */
async function until(events: BridgeEvent[], predicate: (e: BridgeEvent) => boolean) {
  while (!events.some(predicate)) {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

describe('the fake port, sending', () => {
  it('scans, selects and exports with progress, then reports the task ended', async () => {
    const { port, events } = await fake();
    const scanned = await port.scan();
    const selected = await port.select(scanned.session, scanned.selection, []);
    expect(selected.view.issues).toEqual([]);

    const summary = await port.exportArchive(scanned.session, '/tmp/a.carrova', PASS, 'standard');

    expect(summary.bytes).toBeGreaterThan(0);
    expect(engineEvents(events)).toEqual(expect.arrayContaining(['progress', 'finished']));
    expect(events.at(-1)).toEqual({ kind: 'task_ended', session: scanned.session, error: null });
  });

  it('shows selection issues and refuses to export until they are fixed', async () => {
    const { port } = await fake();
    const { session, selection } = await port.scan();
    const keys = selection.items['ssh/keys'];
    if (keys === undefined) {
      throw new Error('the recording has no ssh/keys item');
    }
    expect(keys.sensitive).toBe(true);
    const tick = { kind: 'item', item: 'ssh/keys', included: true } as const;
    const refused = await port.select(session, selection, [tick]);
    expect(refused.view.issues.map((i) => i.item)).toEqual(['ssh/keys']);
    expect(await rejection(port.exportArchive(session, '/tmp/a', PASS, 'standard'))).toMatchObject({
      code: 'invalid_selection',
    });
  });

  it('runs a LAN send: pairing code, peer, progress, finished', async () => {
    const { port, events } = await fake();
    const { session } = await port.scan();
    const report = await port.runSend(session);
    expect(report.outcome).toBe('applied');
    expect(events.find((e) => e.kind === 'pairing_code')).toMatchObject({
      code: { code: '7-acid-lemon', renewed: false },
    });
    expect(engineEvents(events)).toEqual(
      expect.arrayContaining(['peer_connected', 'plan_summary_received', 'finished']),
    );
  });

  it('asks before showing a new code after a wrong one', async () => {
    const { port, events } = await fake('wrong-code');
    const { session } = await port.scan();
    const sending = port.runSend(session);
    await until(events, (e) => e.kind === 'confirm_new_code');
    await port.answerPairing(session, true);
    await sending;
    const codes = events.filter((e) => e.kind === 'pairing_code');
    expect(codes.map((e) => e.code.renewed)).toEqual([false, true]);
  });

  it('stops the send when the user declines a new code', async () => {
    const { port, events } = await fake('wrong-code');
    const { session } = await port.scan();
    const sending = rejection(port.runSend(session));
    await until(events, (e) => e.kind === 'confirm_new_code');
    await port.answerPairing(session, false);
    expect(await sending).toMatchObject({ code: 'pairing_failed' });
  });

  it('cancels a running export', async () => {
    const { port, events } = await fake();
    const { session } = await port.scan();
    const exporting = rejection(port.exportArchive(session, '/tmp/a', PASS, 'standard'));
    await until(events, (e) => e.kind === 'engine');
    await port.cancel(session);
    expect(await exporting).toMatchObject({ code: 'cancelled' });
    expect(events.at(-1)).toMatchObject({ kind: 'task_ended', error: { code: 'cancelled' } });
  });
});

describe('the fake port, receiving', () => {
  it('opens, plans, approves and applies with progress', async () => {
    const { port, events } = await fake();
    const { session } = await port.openArchive('/tmp/a.carrova', 'pass');
    const review = await port.plan(session, OPTIONS);
    expect(review.offered.length).toBeGreaterThan(0);
    await port.approve(session);
    const checklist = await port.apply(session);
    expect(checklist.rollback_available).toBe(true);
    expect(engineEvents(events)).toEqual(
      expect.arrayContaining(['conflict_found', 'action_applied', 'finished']),
    );
  });

  it('keeps the options and approval a resumed session planned with', async () => {
    const { port } = await fake();
    const { session } = await port.openArchive('/tmp/a.carrova', 'pass');
    expect(await port.planOptions(session)).toBeNull();
    await port.plan(session, OPTIONS);
    await port.approve(session);
    await port.resume(session);
    const stored = await port.planOptions(session);
    expect(stored).toEqual(OPTIONS);
    expect((await port.plan(session, OPTIONS)).approval).toBe('given');
  });

  it('rejects a wrong passphrase and a malformed code', async () => {
    const { port } = await fake();
    expect(await rejection(port.openArchive('/tmp/a', WRONG_PASSPHRASE))).toMatchObject({
      code: 'wrong_passphrase',
    });
    expect(await rejection(port.connect('not a code'))).toMatchObject({ code: 'bad_code' });
  });

  it('pauses on a full disk and resumes', async () => {
    const { port, events } = await fake('disk-full');
    const { session } = await port.connect('7-acid-lemon');
    await port.plan(session, OPTIONS);
    await port.approve(session);
    await port.apply(session);
    const names = engineEvents(events);
    expect(names.indexOf('transfer_paused')).toBeLessThan(names.indexOf('transfer_resumed'));
  });

  it('reports a dropped connection and the reconnect', async () => {
    const { port, events } = await fake('reconnect');
    const { session } = await port.connect('7-acid-lemon');
    await port.plan(session, OPTIONS);
    await port.approve(session);
    await port.apply(session);
    expect(engineEvents(events)).toEqual(expect.arrayContaining(['peer_lost', 'peer_connected']));
  });

  it('rolls back a failed apply', async () => {
    const { port, events } = await fake('apply-fails');
    const { session } = await port.openArchive('/tmp/a', 'pass');
    await port.plan(session, OPTIONS);
    await port.approve(session);
    expect(await rejection(port.apply(session))).toMatchObject({ code: 'internal' });
    expect(engineEvents(events)).toContain('rollback_step');
  });

  it('refuses to approve a blocked plan', async () => {
    const { port } = await fake('blocked');
    const { session } = await port.openArchive('/tmp/a', 'pass');
    const review = await port.plan(session, OPTIONS);
    expect(review.plan.blockers).toHaveLength(1);
    expect(await rejection(port.approve(session))).toMatchObject({ code: 'blocked' });
  });

  it('cancels a pairing in progress', async () => {
    const { port } = await fake();
    const connecting = rejection(port.connect('7-acid-lemon'));
    await port.cancelConnect();
    expect(await connecting).toMatchObject({ code: 'cancelled' });
  });
});

describe('the fake port, Manual mode and the file manager', () => {
  it('hands paths sent from the file manager over once (forwarded scenario)', async () => {
    const { port } = await fake('forwarded');
    expect(await port.takeSendRequest()).toEqual([
      '/home/ana/projects/site',
      '/home/ana/notes.txt',
    ]);
    expect(await port.takeSendRequest()).toEqual([]);
    expect(await (await fake()).port.takeSendRequest()).toEqual([]);
  });

  it('writes and removes the file-manager entries on request', async () => {
    const { port } = await fake();
    expect(await port.fileManagerStatus()).toEqual({
      supported: true,
      installed: false,
      skipped: [],
    });
    expect((await port.setFileManagerEntries(true)).installed).toBe(true);
    expect((await port.fileManagerStatus()).installed).toBe(true);
    expect((await port.setFileManagerEntries(false)).installed).toBe(false);
  });

  it("keeps the session's edited picks across the app scan", async () => {
    const { port } = await fake();
    const { session, selection } = await port.startManual();
    const site = 'raw:home/projects/site';
    const env = 'raw:home/work/api/.env';
    const picked = await port.pick(session, selection, [
      '/home/ana/projects/site',
      '/home/ana/work/api/.env',
    ]);
    await port.select(session, picked.scan.selection, [
      { kind: 'remove_pick', item: site },
      { kind: 'item', item: env, included: true },
      { kind: 'confirm_sensitive', item: env, confirmed: true },
    ]);
    const { items } = (await port.scanApps(session)).selection;
    expect(items[site]).toBeUndefined();
    expect(items[env]).toMatchObject({ included: true, confirmed_sensitive: true });
  });

  it('refuses to send after a pick that leaves an included sensitive pick unconfirmed', async () => {
    const { port } = await fake();
    const { session, selection } = await port.startManual();
    const env = 'raw:home/work/api/.env';
    const picked = await port.pick(session, selection, ['/home/ana/work/api/.env']);
    const ticked = await port.select(session, picked.scan.selection, [
      { kind: 'item', item: env, included: true },
    ]);
    const more = await port.pick(session, ticked.selection, ['/home/ana/projects/site']);
    expect(more.scan.view.issues.map((i) => i.item)).toEqual([env]);
    expect(await rejection(port.runSend(session))).toMatchObject({ code: 'invalid_selection' });
  });

  it('offers Manual picks unticked and accept all ticks only the safe ones (picks scenario)', async () => {
    const { port } = await fake('picks');
    const { session } = await port.openArchive('/tmp/a.carrova', 'pass');
    const options = ENGINE_DEFAULTS.plan_options;
    const review = await port.plan(session, options);
    const raw = review.offered.filter((o) => o.key.startsWith('raw:'));
    expect(raw.map((o) => [o.key, o.picked, o.safe_pick])).toEqual([
      ['raw:home/projects/site', false, true],
      ['raw:home/notes.txt', false, true],
      ['raw:home/.ssh/id_ed25519', false, false],
    ]);
    const accepted = await port.editPlan(session, [{ kind: 'accept_safe_picks' }]);
    expect(accepted.offered.filter((o) => o.picked && o.key.startsWith('raw:'))).toHaveLength(2);
    expect(accepted.plan.blockers).toEqual([]);
  });
});

describe('scenario selection', () => {
  it('reads ?fake=<scenario>, falling back to happy for unknown names', () => {
    expect(scenarioFromSearch('?fake=disk-full')).toBe('disk-full');
    expect(scenarioFromSearch('?fake=nonsense')).toBe('happy');
    expect(scenarioFromSearch('')).toBeUndefined();
  });
});
