import { describe, expect, it } from 'vitest';
import { plan } from '@/ipc/fixtures/plan';
import { codeFailure, connectTarget, initialCode } from '@/flows/receive/entry';
import {
  initialReceiveFlow,
  receiveFlowReducer,
  receiveStage,
  type ReceiveFlowAction,
} from '@/flows/receive/receiveFlow';

const DEFAULTS = {
  conflict_rule: 'replace',
  retention: 'until_dismissed',
  include_caches: false,
} as const;

const run = (...actions: ReceiveFlowAction[]) =>
  actions.reduce(receiveFlowReducer, initialReceiveFlow);
const error = (
  code:
    'bad_code' | 'pairing_failed' | 'cannot_connect' | 'network' | 'wrong_passphrase' | 'internal',
) => ({
  code,
  message: 'x',
  hint: null,
});
const reviewing = run(
  { type: 'start', origin: 'lan', defaults: DEFAULTS },
  { type: 'opened', session: 's' },
  { type: 'plan', action: { type: 'ok', value: plan } },
);

describe('receive flow', () => {
  it("resumes from the session's own options; Settings only pre-fill new moves", () => {
    const keepAll = { ...DEFAULTS, conflict_rule: 'keep-both' } as const;
    const stored = { ...plan.plan.options, default_rule: 'skip' } as const;
    const started = { type: 'start', origin: 'file', defaults: keepAll } as const;
    const resumed = run(started, { type: 'resumed', session: 's', options: stored });
    expect(resumed.session).toBe('s');
    expect(resumed.options).toEqual(stored);
    const neverPlanned = run(started, { type: 'resumed', session: 's', options: null });
    expect(neverPlanned.options.default_rule).toBe('keep-both');
  });

  it('reads connect failures: a wrong code, nobody found, anything else', () => {
    expect(codeFailure({ ...initialCode, value: '7-a-b' }, error('pairing_failed'))).toMatchObject({
      status: 'mismatch',
      value: '',
    });
    expect(codeFailure(initialCode, error('cannot_connect')).status).toBe('not-found');
    expect(codeFailure(initialCode, error('network')).status).toBe('not-found');
    expect(codeFailure(initialCode, error('bad_code')).status).toBe('failed');
    expect(codeFailure(initialCode, error('internal')).status).toBe('failed');
  });

  it('connects with the typed address once discovery failed', () => {
    const entry = { ...initialCode, value: '7-acid-lemon', address: '7-acid-lemon@10.0.0.2:47822' };
    expect(connectTarget(entry)).toBe('7-acid-lemon');
    expect(connectTarget({ ...entry, status: 'not-found' })).toBe('7-acid-lemon@10.0.0.2:47822');
  });

  it('marks a wrong passphrase on the file, not as an error banner', () => {
    const state = run(
      { type: 'start', origin: 'file', defaults: DEFAULTS },
      { type: 'opening' },
      { type: 'openFailed', error: error('wrong_passphrase') },
    );
    expect(state.file).toMatchObject({ status: 'wrong', error: null });
  });

  it('moves entry → review → progress → done', () => {
    expect(receiveStage(initialReceiveFlow)).toBe('entry');
    expect(receiveStage(reviewing)).toBe('review');
    const applying = receiveFlowReducer(reviewing, { type: 'apply', action: { type: 'start' } });
    expect(receiveStage(applying)).toBe('progress');
    const checklist = { moved: [], skipped: [], apps_to_install: [], rollback_available: true };
    expect(
      receiveStage(
        receiveFlowReducer(applying, { type: 'apply', action: { type: 'ok', value: checklist } }),
      ),
    ).toBe('done');
    expect(
      receiveStage(
        receiveFlowReducer(applying, {
          type: 'apply',
          action: { type: 'fail', error: error('internal') },
        }),
      ),
    ).toBe('progress');
  });

  it('records answers for their rows; the options stay those of the first plan', () => {
    const skipped = receiveFlowReducer(reviewing, {
      type: 'answer',
      key: 'neovim',
      answer: 'skip',
    });
    expect(skipped.answers['neovim']).toBe('skip');
    expect(skipped.options).toBe(reviewing.options);
  });
});
