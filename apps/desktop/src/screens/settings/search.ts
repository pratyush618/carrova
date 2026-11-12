import type { TFunction } from 'i18next';
import type { SettingsModel } from './model';
import { ROW_TITLE, SECTIONS, type SettingRowId, type SettingsSection } from './sections';

export interface SettingsHit {
  readonly section: SettingsSection;
  readonly row: SettingRowId;
  readonly title: string;
}

/** Rows shown only when the model has them (as their groups render them); every other row always shows. */
const SHOWN_WHEN: Partial<Record<SettingRowId, (model: SettingsModel) => boolean>> = {
  fileManager: (model) => model.fileManager !== null,
  recipes: (model) => model.recipes !== null,
};

/** The shown rows whose title contains `query` (any case), in section order. */
export function searchSettings(query: string, t: TFunction, model: SettingsModel): SettingsHit[] {
  const needle = query.trim().toLowerCase();
  if (needle === '') return [];
  return SECTIONS.flatMap(({ id, rows }) =>
    rows
      .filter((row) => SHOWN_WHEN[row]?.(model) ?? true)
      .map((row) => ({ section: id, row, title: t(ROW_TITLE[row]) }))
      .filter((hit) => hit.title.toLowerCase().includes(needle)),
  );
}
