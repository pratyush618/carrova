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
import type { PickFacts } from '../../bindings/PickFacts';
import type { PickOutcome } from '../../bindings/PickOutcome';
import type { PickRejection } from '../../bindings/PickRejection';
import type { PickResult } from '../../bindings/PickResult';
import type { PlatformReport } from '../../bindings/PlatformReport';
import type { Preset } from '../../bindings/Preset';
import type { Selection } from '../../bindings/Selection';
import type { SelectionEdit } from '../../bindings/SelectionEdit';
import type { SelectionIssue } from '../../bindings/SelectionIssue';
import type { SelectionRow } from '../../bindings/SelectionRow';
import type { SelectionTag } from '../../bindings/SelectionTag';
import type { SelectionView } from '../../bindings/SelectionView';
import type { SemanticRoot } from '../../bindings/SemanticRoot';
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
    case 'allow_plugin':
      return c;
    case 'remove_pick':
      return c;
  }
}

/** `items` without the pick `edit` removes; rejects like the engine when it names no pick. */
function removing(items: Selection['items'], edit: SelectionEdit): Selection['items'] {
  if (edit.kind !== 'remove_pick') return items;
  if (items[edit.item]?.pick == null) {
    raise(bridgeError('invalid_selection', `${edit.item}: not a pick \`${edit.item}\``));
  }
  return Object.fromEntries(Object.entries(items).filter(([key]) => key !== edit.item));
}

/** `allowed` after `edit`: the user's answer for an exporting plugin. */
function consented(allowed: readonly string[], edit: SelectionEdit): string[] {
  if (edit.kind !== 'allow_plugin') return [...allowed];
  const rest = allowed.filter((p) => p !== edit.plugin);
  return edit.allow ? [...rest, edit.plugin] : rest;
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
      Object.entries(removing(current.items, edit)).map(([key, c]) => [key, edited(key, c, edit)]),
    );
    return { ...current, items, confirmed_plugins: consented(current.confirmed_plugins, edit) };
  }, selection);
}

function issuesOf(selection: Selection): SelectionIssue[] {
  const ofItems = Object.entries(selection.items).flatMap(([item, c]) => [
    ...(c.included && c.sensitive && !c.confirmed_sensitive
      ? [{ item, problem: 'sensitive item needs explicit confirmation', plugin: null }]
      : []),
    ...(c.included && !c.force_unmovable && c.unmovable !== null
      ? [{ item, problem: `item is unmovable (${c.unmovable}); force it to include`, plugin: null }]
      : []),
  ]);
  const ofPlugins = selection.export_plugins
    .filter((p) => !selection.confirmed_plugins.includes(p.plugin))
    .flatMap((p) =>
      p.items
        .filter((item) => selection.items[item]?.included === true)
        .map((item) => ({
          item,
          problem: `plugin \`${p.plugin}\` must change its files on this computer before sending`,
          plugin: p.plugin,
        })),
    );
  return [...ofItems, ...ofPlugins];
}

function tagsOf(c: ItemChoice): SelectionTag[] {
  const tags: SelectionTag[] = c.suggested ? [{ kind: 'suggested' }] : [];
  if (c.sensitive) tags.push({ kind: 'sensitive', confirmed: c.confirmed_sensitive });
  if (c.unmovable !== null) {
    tags.push({ kind: 'unmovable', reason: c.unmovable, forced: c.force_unmovable });
  }
  return tags;
}

type Roots = PlatformReport['roots'];

/** The fake's disk under home: what exists there, measured. */
const DISK: Readonly<Record<string, PickFacts>> = {
  projects: { kind: 'dir', size: 52_400_000, files: 1_311 },
  'projects/site': { kind: 'dir', size: 48_000_000, files: 1_204 },
  'projects/site/index.html': { kind: 'file', size: 2_048, files: 1 },
  '.ssh/id_ed25519': { kind: 'file', size: 411, files: 1 },
  'work/api/.env': { kind: 'file', size: 220, files: 1 },
  'notes.txt': { kind: 'file', size: 3_100, files: 1 },
  '.config/starship.toml': { kind: 'file', size: 1_024, files: 1 },
};

/** Secret names (`carrova-core` `secrets::SECRET_NAMES`), checked per path component. */
const isSecret = (name: string): boolean =>
  ['.ssh', '.gnupg', '.aws', '.kube', '.env', '.netrc', '.pgpass'].includes(name) ||
  (name.startsWith('id_') && !name.endsWith('.pub')) ||
  name.endsWith('.pem') ||
  name.endsWith('.key');

const within = (path: string, dir: string): boolean => path.startsWith(`${dir}/`);

interface Resolved {
  readonly root: SemanticRoot;
  readonly rel: string;
  readonly facts: PickFacts;
}

/** `path` as the engine resolves a pick (`locate_pick`, `resolve_pick`), on the fake disk. */
function resolve(path: string, roots: Roots): Resolved | PickRejection {
  const all = Object.entries(roots) as [SemanticRoot, string][];
  const same = all.find(([, dir]) => dir === path);
  if (same) return { kind: 'is_root', root: same[0] };
  const held = all.find(([, dir]) => within(dir, path));
  if (held) return { kind: 'contains_root', root: held[0] };
  const [best] = all
    .filter(([, dir]) => within(path, dir))
    .sort((a, b) => b[1].length - a[1].length);
  if (best === undefined) return { kind: 'outside_roots' };
  const [root, dir] = best;
  const facts = roots.home === undefined ? undefined : DISK[path.slice(roots.home.length + 1)];
  return facts === undefined
    ? { kind: 'not_found' }
    : { root, rel: path.slice(dir.length + 1), facts };
}

/** The pick's path under its root (`raw:home/a/b` → `home/a/b`). */
const pickPath = (key: string): string => key.slice('raw:'.length);

/** `selection` with `path` added as the engine does (`Selection::add_pick`). */
function addPick(selection: Selection, path: string, roots: Roots): [Selection, PickResult] {
  const found = resolve(path, roots);
  if ('kind' in found) return [selection, { kind: 'rejected', reason: found }];
  const { root, rel, facts } = found;
  const key = `raw:${root}/${rel}`;
  if (selection.items[key]) return [selection, { kind: 'duplicate', item: key }];
  const picks = Object.keys(selection.items).filter((k) => selection.items[k]?.pick != null);
  const into = picks.find((k) => within(pickPath(key), pickPath(k)));
  if (into !== undefined) return [selection, { kind: 'absorbed', into }];
  const replaced = picks.filter((k) => within(pickPath(k), pickPath(key)));
  const sensitive = rel.split('/').some(isSecret);
  const choice: ItemChoice = {
    included: !sensitive,
    suggested: false,
    sensitive,
    confirmed_sensitive: false,
    unmovable: null,
    force_unmovable: false,
    excluded_paths: [],
    paths: [`${root}:${rel}`],
    pick: facts,
  };
  const kept = Object.entries(selection.items).filter(([k]) => !replaced.includes(k));
  const items = Object.fromEntries(
    [...kept, [key, choice] as const].sort(([a], [b]) => (a < b ? -1 : 1)),
  );
  return [
    { ...selection, items },
    { kind: 'added', item: key, replaced },
  ];
}

/** `selection` with each of `paths` picked in order, and one outcome per path. */
export function addPicks(
  selection: Selection,
  paths: readonly string[],
  roots: Roots,
): { selection: Selection; outcomes: PickOutcome[] } {
  const outcomes: PickOutcome[] = [];
  const picked = paths.reduce((current, path) => {
    const [next, result] = addPick(current, path, roots);
    outcomes.push({ path, result });
    return next;
  }, selection);
  return { selection: picked, outcomes };
}

/** Where pick `key` is, as the user knows it (`carrova-view` `display`). */
function display(key: string, roots: Roots): string {
  const [root = '', ...rest] = pickPath(key).split('/');
  const full = `${roots[root as SemanticRoot] ?? root}/${rest.join('/')}`;
  return roots.home !== undefined && within(full, roots.home)
    ? `~/${full.slice(roots.home.length + 1)}`
    : full;
}

/** The view of `selection`, with the facts the recorded `base` view knows. */
export function viewOf(
  selection: Selection,
  base: SelectionView,
  roots: Roots = {},
): SelectionView {
  const facts = new Map<string, ItemFacts | null>(base.rows.map((r) => [r.key, r.facts]));
  const entries = Object.entries(selection.items);
  const picks = entries.flatMap(([key, c]) =>
    c.pick == null
      ? []
      : [
          {
            key,
            display: display(key, roots),
            facts: c.pick,
            included: c.included,
            tags: tagsOf(c),
          },
        ],
  );
  const rows: SelectionRow[] = entries
    .filter(([, c]) => c.pick == null)
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
    picks,
    hidden_raw: entries.filter(
      ([key, c]) => key.startsWith('raw:') && c.pick == null && !c.included,
    ).length,
    included: entries.filter(([, c]) => c.included).length,
    included_size:
      rows.reduce((n, r) => n + (r.included ? (r.facts?.size ?? 0) : 0), 0) +
      picks.reduce((n, p) => n + (p.included ? p.facts.size : 0), 0),
    issues: issuesOf(selection),
    plugins: selection.export_plugins,
  };
}
