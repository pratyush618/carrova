import type { ScanResult, Selection } from '../../engine/types';

/** Whether a mode's reply can be sent: answered, and without issues. */
export const usable = (result: ScanResult | null): boolean =>
  result !== null && result.view.issues.length === 0;

/** An empty Manual list, as first handed to the engine. */
export const NO_PICKS: Selection = { items: {}, export_plugins: [], confirmed_plugins: [] };

/** `result` without Manual picks: Auto sends apps only. */
export const appsOnly = (result: ScanResult): ScanResult => ({
  ...result,
  selection: {
    ...result.selection,
    items: Object.fromEntries(Object.entries(result.selection.items).filter(([, c]) => !c.pick)),
  },
  view: { ...result.view, picks: [] },
});
