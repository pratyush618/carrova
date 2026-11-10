/**
 * The fake port's model of the engine's selection rules (`Selection::apply`,
 * `Selection::issues`, `selection_view`), so the gallery, Playwright and unit
 * tests can tick and untick freely. Test double only: never in the production
 * bundle, and `tests/ipc/fake/selection.test.ts` holds it to recorded engine
 * replies. Presets other than `none` and `everything` re-tick the suggestions.
 */
import type { ItemChoice } from '../../bindings/ItemChoice';
import type { ItemFacts } from '../../bindings/ItemFacts';
import type { ItemRoles } from '../../bindings/ItemRoles';
import type { Preset } from '../../bindings/Preset';
import type { Selection } from '../../bindings/Selection';
import type { SelectionEdit } from '../../bindings/SelectionEdit';
import type { SelectionIssue } from '../../bindings/SelectionIssue';
import type { SelectionRow } from '../../bindings/SelectionRow';
import type { SelectionTag } from '../../bindings/SelectionTag';
import type { SelectionView } from '../../bindings/SelectionView';
import { bridgeError, raise } from '../reject';

const rolesOf = (c: ItemChoice): ItemRoles => ({
  ordinary: !c.sensitive && c.unmovable === null,
  movable: c.unmovable === null || c.force_unmovable,
  cache: c.paths.some((p) => p.startsWith('cache:')),
});

const appOfKey = (key: string): string | null =>
  key.startsWith('raw:') ? null : (key.split('/')[0] ?? null);

function preset(c: ItemChoice, which: Preset): boolean {
  switch (which) {
    case 'none':
      return false;
    case 'everything':
      return rolesOf(c).ordinary;
    case 'suggested':
    case 'minimal':
      return c.suggested;
  }
}

function excluding(c: ItemChoice, path: string, excluded: boolean): ItemChoice {
  const rest = c.excluded_paths.filter((p) => p !== path);
  return { ...c, excluded_paths: excluded ? [...rest, path].sort() : rest };
}

/** `c` (of item `key`) after `edit`. */
function edited(key: string, c: ItemChoice, edit: SelectionEdit): ItemChoice {
  switch (edit.kind) {
    case 'preset':
      return { ...c, included: preset(c, edit.preset) };
    case 'app':
      return appOfKey(key) === edit.app && rolesOf(c).ordinary
        ? { ...c, included: edit.included }
        : c;
    case 'item':
      return key === edit.item ? { ...c, included: edit.included } : c;
    case 'path':
      return key === edit.item ? excluding(c, edit.path, !edit.included) : c;
    case 'confirm_sensitive':
      return key === edit.item ? { ...c, confirmed_sensitive: edit.confirmed } : c;
    case 'force_unmovable':
      return key === edit.item ? { ...c, force_unmovable: true } : c;
    case 'caches': {
      const r = rolesOf(c);
      return r.cache && r.movable && !c.sensitive ? { ...c, included: edit.included } : c;
    }
  }
}

/** The item an edit names, if it names one. */
const named = (edit: SelectionEdit): string | null => ('item' in edit ? edit.item : null);

/** `selection` with `edits` applied in order; rejects like the engine on an unknown item. */
export function applyEdits(selection: Selection, edits: readonly SelectionEdit[]): Selection {
  return edits.reduce<Selection>((current, edit) => {
    const item = named(edit);
    if (item !== null && current.items[item] === undefined) {
      raise(bridgeError('invalid_selection', `${item}: unknown item \`${item}\``));
    }
    const items = Object.fromEntries(
      Object.entries(current.items).map(([key, c]) => [key, edited(key, c, edit)]),
    );
    return { items };
  }, selection);
}

function issuesOf(selection: Selection): SelectionIssue[] {
  return Object.entries(selection.items).flatMap(([item, c]) => [
    ...(c.included && c.sensitive && !c.confirmed_sensitive
      ? [{ item, problem: 'sensitive item needs explicit confirmation' }]
      : []),
    ...(c.included && !c.force_unmovable && c.unmovable !== null
      ? [{ item, problem: `item is unmovable (${c.unmovable}); force it to include` }]
      : []),
  ]);
}

function tagsOf(c: ItemChoice): SelectionTag[] {
  const tags: SelectionTag[] = c.suggested ? [{ kind: 'suggested' }] : [];
  if (c.sensitive) tags.push({ kind: 'sensitive', confirmed: c.confirmed_sensitive });
  if (c.unmovable !== null) {
    tags.push({ kind: 'unmovable', reason: c.unmovable, forced: c.force_unmovable });
  }
  return tags;
}

/** The view of `selection`, with the facts the recorded `base` view knows. */
export function viewOf(selection: Selection, base: SelectionView): SelectionView {
  const facts = new Map<string, ItemFacts | null>(base.rows.map((r) => [r.key, r.facts]));
  const entries = Object.entries(selection.items);
  const rows: SelectionRow[] = entries
    .filter(([key, c]) => c.included || !key.startsWith('raw:'))
    .map(([key, c]) => ({
      key,
      included: c.included,
      facts: facts.get(key) ?? null,
      tags: tagsOf(c),
      excluded_paths: c.excluded_paths,
      roles: rolesOf(c),
    }));
  return {
    rows,
    hidden_raw: entries.filter(([key, c]) => key.startsWith('raw:') && !c.included).length,
    included: entries.filter(([, c]) => c.included).length,
    included_size: rows.reduce((n, r) => n + (r.included ? (r.facts?.size ?? 0) : 0), 0),
    issues: issuesOf(selection),
  };
}
