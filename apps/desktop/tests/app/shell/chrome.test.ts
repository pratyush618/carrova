import { describe, expect, it } from 'vitest';
import { i18n } from '@/i18n/i18n';
import { buildChrome } from '@/app/shell/chrome';

describe('buildChrome', () => {
  it('labels the sidebar Home, History, Apps and Settings', () => {
    const chrome = buildChrome({ kind: 'hub', page: 'home' }, i18n.t, () => undefined);
    expect(chrome.hubLinks.map((link) => link.label)).toEqual([
      'Home',
      'History',
      'Apps',
      'Settings',
    ]);
  });
});
