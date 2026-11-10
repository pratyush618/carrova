import type { TFunction } from 'i18next';
import type { ItemRoles } from '../../bindings/ItemRoles';
import type { SelectionRow } from '../../bindings/SelectionRow';
import type { ItemChoice, ScanResult, Selection, SelectionView } from '../../engine/types';
import {
  FOUND_PRESETS,
  type CustomizeModel,
  type FoundApp,
  type FoundGroup,
  type FoundPreset,
  type ItemTag,
  type MoveFacts,
  type RawFolder,
  type SecretItem,
  type TreeApp,
  type TreeItem,
} from '../../screens/send/model';
import { appRef, displayPath, type RootDirs } from '../shared/paths';

type AppReport = ScanResult['report']['apps'][number];
type ItemReport = AppReport['items'][number];

/**
 * One scanned item with the user's current choice for it and the engine's
 * view of that choice (`roles`: how the edits treat it; `tags`: its marks).
 * Every view below reads one `ScanResult` (an engine reply), so the report,
 * the selection and its view always agree.
 */
interface Entry {
  readonly key: string;
  readonly item: ItemReport;
  readonly choice: ItemChoice;
  readonly roles: ItemRoles;
  readonly tags: SelectionRow['tags'];
}

function entries(app: AppReport, result: ScanResult): Entry[] {
  const rows = new Map(result.view.rows.map((r) => [r.key, r]));
  return app.items.flatMap((item) => {
    const key = `${app.id}/${item.id}`;
    const choice = result.selection.items[key];
    const row = rows.get(key);
    return choice === undefined || row === undefined
      ? []
      : [{ key, item, choice, roles: row.roles, tags: row.tags }];
  });
}

/** The selection key of a folder no recipe covers. */
const rawKey = (r: ScanResult['report']['unrecognised'][number]): string =>
  `raw:${r.root}/${r.rel}`;

const detected = (result: ScanResult): AppReport[] =>
  result.report.apps.filter((a) => a.detected && a.items.length > 0);

/** The one tag shown next to an item: the most important of the engine's marks. */
export function tagOf(tags: SelectionRow['tags']): ItemTag | null {
  const order: readonly ItemTag['kind'][] = ['unmovable', 'sensitive', 'suggested'];
  const ranked = [...tags].sort((a, b) => order.indexOf(a.kind) - order.indexOf(b.kind));
  return ranked[0] ?? null;
}

function foundApp(app: AppReport, result: ScanResult): FoundApp {
  const ordinary = entries(app, result).filter((e) => e.roles.ordinary);
  return {
    ...appRef(app.id, app.name),
    what: ordinary.map((e) => e.item.label).join(', '),
    files: ordinary.reduce((n, e) => n + e.item.file_count, 0),
    bytes: ordinary.reduce((n, e) => n + e.item.size, 0),
    included: ordinary.some((e) => e.choice.included),
  };
}

/** One row per detected app (with what its row ticks: its ordinary items). */
export const foundApps = (result: ScanResult): FoundApp[] =>
  detected(result).map((app) => foundApp(app, result));

/**
 * "Here's what we found", grouped by category in the scan's order. Apps with
 * nothing but secrets are left out: their secrets are asked on their own.
 */
export function foundGroups(result: ScanResult): FoundGroup[] {
  const groups = new Map<string, FoundApp[]>();
  for (const app of detected(result)) {
    if (!entries(app, result).some((e) => e.roles.ordinary)) continue;
    const list = groups.get(app.category) ?? [];
    list.push(foundApp(app, result));
    groups.set(app.category, list);
  }
  return [...groups].map(([category, apps]) => ({ category, apps }));
}

/** Keys and secrets: movable sensitive items, each brought only after its own confirm. */
export function secretItems(result: ScanResult): SecretItem[] {
  const dirs = result.report.platform.roots;
  return detected(result).flatMap((app) =>
    entries(app, result)
      .filter((e) => e.choice.sensitive && e.roles.movable)
      .map((e) => ({
        key: e.key,
        app: appRef(app.id, app.name),
        label: e.item.label,
        why: e.item.sensitive_reason,
        paths: e.item.paths.map((p) => displayPath(p, dirs)),
        bytes: e.item.size,
        brought: e.choice.included && e.choice.confirmed_sensitive,
      })),
  );
}

/** Found apps and secrets whose app (or, for a secret, its label) contains `filter`. */
export function filterFound(
  groups: readonly FoundGroup[],
  secrets: readonly SecretItem[],
  filter: string,
): { groups: FoundGroup[]; secrets: SecretItem[] } {
  const needle = filter.trim().toLocaleLowerCase();
  const hit = (...names: string[]) => names.some((n) => n.toLocaleLowerCase().includes(needle));
  return {
    groups: groups
      .map((g) => ({ ...g, apps: g.apps.filter((a) => hit(a.name)) }))
      .filter((g) => g.apps.length > 0),
    secrets: secrets.filter((s) => hit(s.app.name, s.label)),
  };
}

/** What each preset ticks, as `carrova_core::select::Selection::apply_preset` does. */
const PRESET_TICKS: Readonly<Record<FoundPreset, (c: ItemChoice) => boolean>> = {
  suggested: (c) => c.suggested,
  everything: (c) => !c.sensitive && c.unmovable === null,
  none: () => false,
};

/** The preset `selection` still matches, or `null` once edited. Keys don't count: they're asked one by one. */
export function activePreset(selection: Selection): FoundPreset | null {
  const items = Object.values(selection.items).filter((c) => !c.sensitive);
  return FOUND_PRESETS.find((p) => items.every((c) => c.included === PRESET_TICKS[p](c))) ?? null;
}

/** Cache items the "Include caches" switch acts on. */
const cacheEntries = (flat: readonly Entry[]): Entry[] =>
  flat.filter((e) => e.roles.cache && !e.choice.sensitive);

/** Every caches item is ticked (and there is at least one). */
const cachesIncluded = (caches: readonly Entry[]): boolean =>
  caches.length > 0 && caches.every((e) => e.choice.included);

/** The rail's facts: apps ticked of those found, bytes ticked, keys brought, caches. */
export function moveFacts(result: ScanResult): MoveFacts {
  const apps = foundGroups(result).flatMap((g) => g.apps);
  const flat = detected(result).flatMap((app) => entries(app, result));
  const raw = result.report.unrecognised.filter(
    (r) => result.selection.items[rawKey(r)]?.included === true,
  );
  return {
    apps: apps.filter((a) => a.included).length,
    totalApps: apps.length,
    bytes:
      flat.filter((e) => e.choice.included).reduce((n, e) => n + e.item.size, 0) +
      raw.reduce((n, r) => n + r.size, 0),
    keys: secretItems(result).filter((s) => s.brought).length,
    caches: cachesIncluded(cacheEntries(flat)),
  };
}

/** Why the selection can't be used yet, from the engine's view of it. */
export const selectionIssues = (view: SelectionView): string[] => view.issues.map((i) => i.problem);

function treeItem(e: Entry, dirs: RootDirs): TreeItem {
  const { paths, excluded_paths: excluded } = e.choice;
  const [only] = paths;
  return {
    key: e.key,
    label: e.item.label,
    detail:
      paths.length === 1 && only !== undefined
        ? { kind: 'path', path: displayPath(only, dirs) }
        : { kind: 'files', count: e.item.file_count },
    bytes: e.item.size,
    picked: e.choice.included,
    tag: tagOf(e.tags),
    paths:
      paths.length > 1
        ? paths.map((p) => ({
            path: p,
            display: displayPath(p, dirs),
            picked: !excluded.includes(p),
          }))
        : [],
  };
}

/** UI-only state of the Customize sheet. */
export interface CustomizeUi {
  readonly filter: string;
  readonly expanded: readonly string[];
}

/** The full tree: apps → items → paths, with caches and raw folders summarised. */
export function customizeModel(
  result: ScanResult,
  ui: CustomizeUi,
  minRawBytes: number,
): CustomizeModel {
  const dirs = result.report.platform.roots;
  const needle = ui.filter.trim().toLocaleLowerCase();
  const all = detected(result).map((app) => ({ app, list: entries(app, result) }));
  const flat = all.flatMap((a) => a.list);
  const picked = flat.filter((e) => e.choice.included);
  const caches = cacheEntries(flat);
  const apps: TreeApp[] = all
    .map(({ app, list }) => ({
      ...appRef(app.id, app.name),
      bytes: list.reduce((n, e) => n + e.item.size, 0),
      expanded: ui.expanded.includes(app.id) || needle !== '',
      items: list
        .filter(
          (e) =>
            needle === '' || `${app.name} ${e.item.label}`.toLocaleLowerCase().includes(needle),
        )
        .map((e) => treeItem(e, dirs)),
    }))
    .filter((a) => a.items.length > 0);
  return {
    filter: ui.filter,
    apps,
    unrecognised: {
      count: result.report.unrecognised.filter((r) => r.size >= minRawBytes).length,
      minBytes: minRawBytes,
    },
    includeCaches: cachesIncluded(caches),
    cacheBytes: caches.reduce((n, e) => n + e.item.size, 0),
    picked: {
      files: picked.reduce((n, e) => n + e.item.file_count, 0),
      totalFiles: flat.reduce((n, e) => n + e.item.file_count, 0),
      bytes: picked.reduce((n, e) => n + e.item.size, 0),
    },
  };
}

/** Raw folders at least `minBytes` big, ticked or not. */
export function rawFolders(result: ScanResult, minBytes: number, t: TFunction): RawFolder[] {
  const dirs = result.report.platform.roots;
  return result.report.unrecognised
    .filter((r) => r.size >= minBytes)
    .map((r) => {
      const key = rawKey(r);
      return {
        key,
        path: displayPath(`${r.root}:${r.rel}`, dirs),
        hint: t('count.files', { count: r.file_count }),
        bytes: r.size,
        picked: result.selection.items[key]?.included ?? false,
      };
    });
}
