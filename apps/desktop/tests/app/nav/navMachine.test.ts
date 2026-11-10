import { describe, expect, it } from 'vitest';
import { flowLength, flowStart, stepIndex } from '@/app/nav/flows';
import { firstRun, navigate, returning, type Nav, type NavEvent } from '@/app/nav/navMachine';

const run = (from: Nav, ...events: NavEvent[]): Nav => events.reduce(navigate, from);

describe('flows', () => {
  it('starts every flow at its first step', () => {
    expect(flowStart('send')).toEqual({ flow: 'send', step: 'found' });
    expect(flowStart('open')).toEqual({ flow: 'open', step: 'file' });
  });

  it('counts steps from the one flow table', () => {
    expect(flowLength('send')).toBe(4);
    expect(flowLength('save')).toBe(3);
    expect(stepIndex({ flow: 'receive', step: 'progress' })).toBe(2);
  });
});

describe('navigate', () => {
  it('leaves the welcome screen for Home exactly once', () => {
    const home = run(firstRun, { type: 'finishWelcome' });
    expect(home.view).toEqual({ kind: 'hub', page: 'home' });
    expect(navigate(home, { type: 'finishWelcome' })).toBe(home);
  });

  it('walks the send flow forward and lands Home after the last step', () => {
    const steps = [returning];
    for (let i = 0; i < 4; i++) {
      const prev = steps.at(-1) ?? returning;
      steps.push(navigate(prev, i === 0 ? { type: 'startFlow', flow: 'send' } : { type: 'next' }));
    }
    expect(
      steps.slice(1).map((n) => (n.view.kind === 'flow' ? n.view.at.step : n.view.kind)),
    ).toEqual(['found', 'code', 'live', 'done']);
    expect(navigate(steps.at(-1) ?? returning, { type: 'next' }).view).toEqual({
      kind: 'hub',
      page: 'home',
    });
  });

  it('goes back a step, and back from the first step goes Home', () => {
    const atCode = run(returning, { type: 'startFlow', flow: 'receive' }, { type: 'next' });
    expect(atCode.view).toEqual({ kind: 'flow', at: { flow: 'receive', step: 'review' } });
    expect(run(atCode, { type: 'back' }, { type: 'back' }).view).toEqual({
      kind: 'hub',
      page: 'home',
    });
  });

  it('closes a flow from any step', () => {
    const mid = run(returning, { type: 'startFlow', flow: 'save' }, { type: 'next' });
    expect(navigate(mid, { type: 'closeFlow' })).toEqual(returning);
  });

  it('ignores flow steps outside a flow', () => {
    expect(navigate(returning, { type: 'next' })).toBe(returning);
    expect(navigate(returning, { type: 'closeFlow' })).toBe(returning);
  });

  it('stacks overlays and closes the top one first', () => {
    const stacked = run(
      returning,
      { type: 'startFlow', flow: 'send' },
      { type: 'openOverlay', overlay: { kind: 'customize' } },
      { type: 'openOverlay', overlay: { kind: 'unrecognised' } },
    );
    expect(stacked.overlays).toEqual([{ kind: 'customize' }, { kind: 'unrecognised' }]);
    expect(navigate(stacked, { type: 'closeOverlay' }).overlays).toEqual([{ kind: 'customize' }]);
  });

  it('drops every overlay when the view changes', () => {
    const withDialog = navigate(returning, {
      type: 'openOverlay',
      overlay: { kind: 'undo-confirm' },
    });
    const undone = navigate(withDialog, {
      type: 'enterFlow',
      at: { flow: 'undo', step: 'result' },
    });
    expect(undone).toEqual({
      view: { kind: 'flow', at: { flow: 'undo', step: 'result' } },
      overlays: [],
    });
  });
});
