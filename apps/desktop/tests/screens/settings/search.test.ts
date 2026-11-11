import { describe, expect, it } from 'vitest';
import { i18n } from '@/i18n/i18n';
import { searchSettings } from '@/screens/settings/search';

const t = i18n.t.bind(i18n);

describe('searchSettings', () => {
  it('finds the notifications row for "wake"', () => {
    expect(searchSettings('wake', t)).toEqual([
      { section: 'notifications', row: 'wakeScreen', title: 'Wake the screen for send requests' },
    ]);
  });

  it('matches any part of a title, ignoring case and spacing around the query', () => {
    expect(searchSettings('  THEME ', t).map((r) => r.row)).toEqual(['theme']);
  });

  it('returns nothing for an empty query or a hidden section', () => {
    expect(searchSettings('   ', t)).toEqual([]);
    expect(searchSettings('zzz', t)).toEqual([]);
  });
});
