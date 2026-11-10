import { describe, expect, it } from 'vitest';
import { emptySession } from '@/app/state/session';
import { chipStates, transferModel, transferPhase } from '@/flows/shared/transferView';

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
});
