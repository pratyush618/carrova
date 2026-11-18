import { describe, expect, it } from 'vitest';
import type { View } from '@/app/nav/navMachine';
import { quietNote } from '@/app/shell/quietNote';
import type { NoteAlert } from '@/app/state/link';
import type { LinkedDeviceView } from '@/engine/types';
import { pixel } from '@/screens/devices/fixtures';

const text: NoteAlert = { device: 'p8', key: 'k', locked: false, smsApp: true, n: 1 };
const messages: View = { kind: 'hub', page: 'messages' };
const sharing = (effective: 'on' | 'off'): LinkedDeviceView => ({
  ...pixel,
  features: [
    ...pixel.features,
    { feature: 'messages', group: 'quiet', local: 'on', remote: effective, effective },
  ],
});

describe('quietNote', () => {
  it('holds back a text while its phone’s Messages is in front', () => {
    expect(quietNote(text, messages, [sharing('on')], true)).toBe(true);
  });

  it.each<[string, NoteAlert, View, LinkedDeviceView[], boolean]>([
    ['another app’s note', { ...text, smsApp: false }, messages, [sharing('on')], true],
    ['the window not in front', text, messages, [sharing('on')], false],
    ['another page', text, { kind: 'hub', page: 'home' }, [sharing('on')], true],
    ['Messages off on the phone', text, messages, [sharing('off')], true],
    [
      'Messages showing another phone',
      { ...text, device: 'other' },
      messages,
      [sharing('on')],
      true,
    ],
    ['no phone sharing texts', text, messages, [pixel], true],
    [
      'a phone without SMS access',
      text,
      messages,
      [{ ...sharing('on'), missing: ['messages'] }],
      true,
    ],
  ])('pops up for %s', (_, note, view, devices, focused) => {
    expect(quietNote(note, view, devices, focused)).toBe(false);
  });
});
