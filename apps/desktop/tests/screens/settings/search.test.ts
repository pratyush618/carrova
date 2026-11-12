import { describe, expect, it } from 'vitest';
import { i18n } from '@/i18n/i18n';
import { settingsModel, settingsNoFileManager } from '@/screens/settings/fixtures';
import { searchSettings } from '@/screens/settings/search';

const t = i18n.t.bind(i18n);

describe('searchSettings', () => {
  it('finds the notifications row for "wake"', () => {
    expect(searchSettings('wake', t, settingsModel)).toEqual([
      { section: 'notifications', row: 'wakeScreen', title: 'Wake the screen for send requests' },
    ]);
  });

  it('matches any part of a title, ignoring case and spacing around the query', () => {
    expect(searchSettings('  THEME ', t, settingsModel).map((r) => r.row)).toEqual(['theme']);
  });

  it('returns nothing for an empty query or a hidden section', () => {
    expect(searchSettings('   ', t, settingsModel)).toEqual([]);
    expect(searchSettings('zzz', t, settingsModel)).toEqual([]);
  });

  it('leaves out rows this computer does not show', () => {
    const rows = (model: typeof settingsModel, query: string) =>
      searchSettings(query, t, model).map((r) => r.row);
    expect(rows(settingsModel, 'file manager')).toContain('fileManager');
    expect(rows(settingsNoFileManager, 'file manager')).not.toContain('fileManager');
    expect(rows({ ...settingsModel, recipes: null }, 'recipe')).not.toContain('recipes');
  });
});
