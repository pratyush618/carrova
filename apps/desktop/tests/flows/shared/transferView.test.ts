import { describe, expect, it } from 'vitest';
import { emptySession } from '@/app/state/session';
import {
  addSample,
  chipStates,
  rateOf,
  transferModel,
  transferPhase,
} from '@/flows/shared/transferView';

const apps = [
  { id: 'big', glyph: 'big', name: 'Big', bytes: 900 },
  { id: 'small', glyph: 'sma', name: 'Small', bytes: 100 },
];

describe('transfer view', () => {
  it('fills chips small-first, like the engine moves them', () => {
    expect(chipStates(apps, 0.5).map((a) => [a.id, a.state, a.bytesDone])).toEqual([
      ['small', 'done', 0],
      ['big', 'moving', 400],
    ]);
    expect(chipStates(apps, 0).every((a) => a.state === 'waiting')).toBe(true);
  });

  it('shows a disk-full pause over a lost link, and the shortfall to free', () => {
    const session = {
      ...emptySession('s'),
      link: 'retrying' as const,
      paused: { kind: 'low_disk_space' as const, fs: '/home', available: 100, needed: 500 },
    };
    expect(transferPhase(session)).toBe('paused');
    expect(transferModel('receive', session, apps, 'old').shortfall).toBe(400);
    expect(transferPhase({ ...session, paused: null })).toBe('reconnecting');
  });

  it('enters the apply phase from apply progress', () => {
    const session = { ...emptySession('s'), phase: 'apply' as const, done: 1, total: 2 };
    expect(transferModel('receive', session, apps, 'old')).toMatchObject({
      phase: 'applying',
      done: 1,
      total: 2,
    });
  });

  it('measures the rate over the last seconds and restarts when the count goes back', () => {
    const samples = [0, 1_000, 2_000, 8_000].reduce(
      (list, at) => addSample(list, { at, done: at * 10 }),
      addSample([], { at: 0, done: 0 }),
    );
    expect(samples.map((s) => s.at)).toEqual([8_000]);
    const steady = addSample(addSample([], { at: 0, done: 0 }), { at: 2_000, done: 4_000 });
    expect(rateOf(steady)).toBe(2_000);
    expect(rateOf(addSample([], { at: 0, done: 0 }))).toBeNull();
    expect(addSample(steady, { at: 2_500, done: 10 })).toEqual([{ at: 2_500, done: 10 }]);
  });

  it('turns the rate into time left while moving, and hides both when stopped', () => {
    const moving = { ...emptySession('s'), done: 100, total: 1_100 };
    expect(transferModel('receive', moving, apps, 'old', { rate: 50 })).toMatchObject({
      bytesPerSecond: 50,
      secondsLeft: 20,
      route: null,
    });
    const held = { ...moving, paused: { kind: 'by_user' as const } };
    expect(transferModel('receive', held, apps, 'old', { rate: 50 })).toMatchObject({
      bytesPerSecond: null,
      secondsLeft: null,
    });
  });
});
