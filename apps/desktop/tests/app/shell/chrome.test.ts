import { describe, expect, it } from 'vitest';
import { i18n } from '@/i18n/i18n';
import { buildChrome } from '@/app/shell/chrome';
import type { View } from '@/app/nav/navMachine';

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

  it('adds Calls after Messages once a phone shares its calls, lit on its calls page', () => {
    const ids = (view: View, calls: boolean) =>
      buildChrome(view, i18n.t, () => undefined, null, undefined, false, true, calls).hubLinks;
    const calls: View = { kind: 'calls', id: 'p1' };
    expect(
      ids(calls, true)
        .map((link) => link.id)
        .slice(0, 4),
    ).toEqual(['home', 'nearby', 'messages', 'calls']);
    expect(ids(calls, true).find((link) => link.current)?.id).toBe('calls');
    expect(ids({ kind: 'hub', page: 'calls' }, true).find((link) => link.current)?.label).toBe(
      'Calls',
    );
    // Calls off: no link; the page stays under Nearby, where it was opened.
    expect(ids(calls, false).map((link) => link.id)).not.toContain('calls');
    expect(ids(calls, false).find((link) => link.current)?.id).toBe('nearby');
  });
});
