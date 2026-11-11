import { describe, expect, it } from 'vitest';
import { FLOWS, flowStart, stepIndex } from '@/app/nav/flows';
import { atHome, navigate, type Nav, type NavEvent } from '@/app/nav/navMachine';

const run = (from: Nav, ...events: NavEvent[]): Nav => events.reduce(navigate, from);

describe('flows', () => {
  it('starts every flow at its first step', () => {
    expect(flowStart('send')).toEqual({ flow: 'send', step: 'found' });
    expect(flowStart('open')).toEqual({ flow: 'open', step: 'file' });
  });

  it('counts steps from the one flow table', () => {
    expect(FLOWS.send).toHaveLength(4);
    expect(FLOWS.save).toHaveLength(3);
    expect(stepIndex({ flow: 'receive', step: 'progress' })).toBe(2);
  });
});

describe('navigate', () => {
  it('opens on Home', () => {
    expect(atHome).toEqual({ view: { kind: 'hub', page: 'home' }, origin: 'home', overlays: [] });
  });

  it('walks the send flow forward and lands Home after the last step', () => {
    const steps = [atHome];
    for (let i = 0; i < 4; i++) {
      const prev = steps.at(-1) ?? atHome;
      steps.push(navigate(prev, i === 0 ? { type: 'startFlow', flow: 'send' } : { type: 'next' }));
    }
    expect(
      steps.slice(1).map((n) => (n.view.kind === 'flow' ? n.view.at.step : n.view.kind)),
    ).toEqual(['found', 'code', 'live', 'done']);
    expect(navigate(steps.at(-1) ?? atHome, { type: 'next' }).view).toEqual({
      kind: 'hub',
      page: 'home',
    });
  });

  it('goes back a step, and back from the first step goes Home', () => {
    const atCode = run(atHome, { type: 'startFlow', flow: 'receive' }, { type: 'next' });
    expect(atCode.view).toEqual({ kind: 'flow', at: { flow: 'receive', step: 'review' } });
    expect(run(atCode, { type: 'back' }, { type: 'back' }).view).toEqual({
      kind: 'hub',
      page: 'home',
    });
  });

  it('closes a flow from any step', () => {
    const mid = run(atHome, { type: 'startFlow', flow: 'save' }, { type: 'next' });
    expect(navigate(mid, { type: 'closeFlow' })).toEqual(atHome);
  });

  it('returns to the page a flow began on when it is closed or backed out of', () => {
    const history = run(atHome, { type: 'openHub', page: 'history' });
    const send = run(history, { type: 'startFlow', flow: 'send' });
    const histView = { kind: 'hub', page: 'history' };
    expect(run(send, { type: 'closeFlow' }).view).toEqual(histView);
    expect(run(send, { type: 'back' }).view).toEqual(histView);
    expect(run(send, { type: 'next' }, { type: 'closeFlow' }).view).toEqual(histView);
  });

  it('ignores flow steps outside a flow', () => {
    expect(navigate(atHome, { type: 'next' })).toBe(atHome);
    expect(navigate(atHome, { type: 'closeFlow' })).toBe(atHome);
  });

  it('stacks overlays and closes the top one first', () => {
    const stacked = run(
      atHome,
      { type: 'startFlow', flow: 'send' },
      { type: 'openOverlay', overlay: { kind: 'customize' } },
      { type: 'openOverlay', overlay: { kind: 'unrecognised' } },
    );
    expect(stacked.overlays).toEqual([{ kind: 'customize' }, { kind: 'unrecognised' }]);
    expect(navigate(stacked, { type: 'closeOverlay' }).overlays).toEqual([{ kind: 'customize' }]);
  });

  it('drops every overlay when the view changes', () => {
    const withDialog = navigate(atHome, {
      type: 'openOverlay',
      overlay: { kind: 'undo-confirm' },
    });
    const undone = navigate(withDialog, {
      type: 'enterFlow',
      at: { flow: 'undo', step: 'result' },
    });
    expect(undone).toEqual({
      view: { kind: 'flow', at: { flow: 'undo', step: 'result' } },
      origin: 'home',
      overlays: [],
    });
  });

  it('opens Settings on a section and back returns to the page it was opened from', () => {
    const history = navigate(atHome, { type: 'openHub', page: 'history' });
    const settings = navigate(history, { type: 'openHub', page: 'settings' });
    expect(settings.view).toEqual({ kind: 'hub', page: 'settings', section: 'general' });
    const moves = navigate(settings, { type: 'openHub', page: 'settings', section: 'moves' });
    expect(moves.view).toEqual({ kind: 'hub', page: 'settings', section: 'moves' });
    expect(navigate(moves, { type: 'back' }).view).toEqual({ kind: 'hub', page: 'history' });
  });
});
