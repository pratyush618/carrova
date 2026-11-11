import type { TFunction } from 'i18next';
import { ROW_TITLE, SECTIONS, type SettingRowId, type SettingsSection } from './sections';

export interface SettingsHit {
  readonly section: SettingsSection;
  readonly row: SettingRowId;
  readonly title: string;
}

/** The rows whose title contains `query` (any case), in section order. */
export function searchSettings(query: string, t: TFunction): SettingsHit[] {
  const needle = query.trim().toLowerCase();
  if (needle === '') return [];
  return SECTIONS.flatMap(({ id, rows }) =>
    rows
      .map((row) => ({ section: id, row, title: t(ROW_TITLE[row]) }))
      .filter((hit) => hit.title.toLowerCase().includes(needle)),
  );
}
