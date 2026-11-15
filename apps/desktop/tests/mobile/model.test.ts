import { describe, expect, it } from 'vitest';
import { openedIntent, openedIntents } from '@/mobile/model';

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

  it('ignores shared text and anything else', () => {
    expect(openedIntent('data:text/plain,hello')).toBeNull();
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
      { kind: 'clipboard' },
    ]);
  });
});
