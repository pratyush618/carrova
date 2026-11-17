import { describe, expect, it } from 'vitest';
import type { FeatureState, LinkedDeviceView } from '@/engine/types';
import { openedIntent, openedIntents, unreachable } from '@/mobile/model';

describe('a link the phone app was opened with', () => {
  it('asks to send the clipboard from the tile or the notification', () => {
    expect(openedIntent('carrova://send-clipboard')).toEqual({ kind: 'clipboard' });
  });

  it('turns a shared file into its decoded path', () => {
    expect(
      openedIntent('file:///data/user/0/org.byteveda.Carrova/files/cache/shared/1/IMG%202041.jpg'),
    ).toEqual({
      kind: 'files',
      paths: ['/data/user/0/org.byteveda.Carrova/files/cache/shared/1/IMG 2041.jpg'],
    });
  });

  it('hands a shared web link off', () => {
    expect(openedIntent('https://carrova.org/docs')).toEqual({
      kind: 'handoff',
      url: 'https://carrova.org/docs',
    });
  });

  it('decodes shared text, any script', () => {
    expect(openedIntent('data:text/plain,caf%C3%A9%20%E2%9C%93%0Anext%20line')).toEqual({
      kind: 'text',
      text: 'café ✓\nnext line',
    });
  });

  it('ignores anything else', () => {
    expect(openedIntent('carrova://pair?v=1')).toBeNull();
  });
});

describe('the links one opening brought', () => {
  it('sends one share of several files together, in order', () => {
    expect(
      openedIntents([
        'file:///shared/1/a.jpg',
        'file:///shared/1/b.jpg',
        'data:text/plain,hi',
        'carrova://send-clipboard',
      ]),
    ).toEqual([
      { kind: 'files', paths: ['/shared/1/a.jpg', '/shared/1/b.jpg'] },
      { kind: 'text', text: 'hi' },
      { kind: 'clipboard' },
    ]);
  });
});

/** A computer with `clipboard` set `local` here and `remote` there. */
const computer = (
  state: LinkedDeviceView['state'],
  local: FeatureState,
  remote: FeatureState | null,
): LinkedDeviceView => ({
  id: 'd2',
  name: 'Desk PC',
  os: 'linux',
  fingerprint: 'B04E·88D1',
  state,
  features: [
    {
      feature: 'clipboard',
      group: 'quiet',
      local,
      remote,
      effective: local === 'off' || remote === null || remote === 'off' ? 'off' : 'on',
    },
  ],
  missing: [],
});

describe('why a linked computer cannot get a send now', () => {
  it('can when linked and on on both sides', () => {
    expect(unreachable(computer('linked', 'on', 'on'), 'clipboard')).toBeNull();
  });

  it('is off on this phone first: the one thing the user can change here', () => {
    expect(unreachable(computer('away', 'off', null), 'clipboard')).toBe('offHere');
  });

  it('is away or connecting while the link is down', () => {
    expect(unreachable(computer('away', 'on', null), 'clipboard')).toBe('away');
    expect(unreachable(computer('connecting', 'on', null), 'clipboard')).toBe('connecting');
  });

  it('is off on the computer once its setting is known', () => {
    expect(unreachable(computer('linked', 'on', 'off'), 'clipboard')).toBe('offThere');
  });

  it('is still connecting while linked but the computer has not said yet', () => {
    expect(unreachable(computer('linked', 'on', null), 'clipboard')).toBe('connecting');
  });
});
