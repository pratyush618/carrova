import { describe, expect, it } from 'vitest';
import { i18n } from '@/i18n/i18n';
import { buildChrome } from '@/app/shell/chrome';

describe('buildChrome', () => {
  it('labels the sidebar Home, Nearby, History, Apps and Settings', () => {
    const chrome = buildChrome({ kind: 'hub', page: 'home' }, i18n.t, () => undefined);
    expect(chrome.hubLinks.map((link) => link.label)).toEqual([
      'Home',
      'Nearby',
      'History',
      'Apps',
      'Settings',
    ]);
  });

  it('lights Nearby only while others can find this computer', () => {
    const live = (visible: boolean) =>
      buildChrome({ kind: 'hub', page: 'home' }, i18n.t, () => undefined, null, undefined, visible)
        .hubLinks.filter((link) => link.live === true)
        .map((link) => link.id);
    expect(live(true)).toEqual(['nearby']);
    expect(live(false)).toEqual([]);
  });

  it('adds Messages after Nearby once a phone shares its texts', () => {
    const chrome = buildChrome(
      { kind: 'hub', page: 'messages' },
      i18n.t,
      () => undefined,
      null,
      undefined,
      false,
      true,
    );
    expect(chrome.hubLinks.map((link) => link.id).slice(0, 3)).toEqual([
      'home',
      'nearby',
      'messages',
    ]);
    expect(chrome.hubLinks.find((link) => link.current)?.label).toBe('Messages');
  });
});
