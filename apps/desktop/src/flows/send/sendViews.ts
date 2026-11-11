import type { TFunction } from 'i18next';
import type { ItemRoles } from '../../bindings/ItemRoles';
import type { SelectionRow } from '../../bindings/SelectionRow';
import type {
  ItemChoice,
  PickOutcome,
  ScanResult,
  Selection,
  SelectionView,
} from '../../engine/types';
import {
  FOUND_PRESETS,
  type CustomizeModel,
  type FoundApp,
  type FoundFilter,
  type FoundGroup,
  type AbsorbedPick,
  type FoundPreset,
  type ItemTag,
  type KeyConfirmModel,
  type ManualPick,
  type MoveFacts,
  type RawFolder,
  type RejectedPick,
  type SecretItem,
  type TreeApp,
  type TreeItem,
} from '../../screens/send/model';
import type { PluginConsentModel } from '../../screens/receive/model';
import { sensitiveWhy } from '../../screens/shared/sensitiveWhy';
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
    suggested: ordinary.some((e) => e.choice.suggested),
  };
}

/** One row per detected app (with what its row ticks: its ordinary items). */
export const foundApps = (result: ScanResult): FoundApp[] =>
  detected(result).map((app) => foundApp(app, result));

/** More found apps than this and every category starts as one line. */
const COLLAPSE_ABOVE = 30;

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
  const collapsed = [...groups.values()].reduce((n, apps) => n + apps.length, 0) > COLLAPSE_ABOVE;
  return [...groups].map(([category, apps]) => ({ category, collapsed, apps }));
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
        why: sensitiveWhy(e.key),
        paths: e.item.paths.map((p) => displayPath(p, dirs)),
        keys: e.item.keys.map((k) => ({ ...k, path: displayPath(k.path, dirs) })),
        bytes: e.item.size,
        brought: e.choice.included && e.choice.confirmed_sensitive,
      })),
  );
}

/** Which found apps each view keeps. "Largest" keeps all (it only reorders). */
const FOUND_VIEWS: Readonly<Record<FoundFilter, (app: FoundApp) => boolean>> = {
  all: () => true,
  ticked: (a) => a.included,
  changed: (a) => a.included !== a.suggested,
  largest: () => true,
};

const bySize = (a: FoundApp, b: FoundApp): number => b.bytes - a.bytes;

/** Each group's apps biggest first, the group holding the biggest app first. */
const largestFirst = (groups: readonly FoundGroup[]): FoundGroup[] =>
  groups
    .map((g) => ({ ...g, apps: g.apps.toSorted(bySize) }))
    .sort((a, b) => (b.apps[0]?.bytes ?? 0) - (a.apps[0]?.bytes ?? 0));

/**
 * Found apps and secrets whose app (or, for a secret, its label) contains `filter`,
 * the apps narrowed to `view`. Narrowing opens every group: you asked to see them.
 */
export function filterFound(
  groups: readonly FoundGroup[],
  secrets: readonly SecretItem[],
  filter: string,
  view: FoundFilter,
): { groups: FoundGroup[]; secrets: SecretItem[] } {
  const needle = filter.trim().toLocaleLowerCase();
  const hit = (...names: string[]) => names.some((n) => n.toLocaleLowerCase().includes(needle));
  const narrowed = needle !== '' || view !== 'all';
  const shown = groups
    .map((g) => ({
      ...g,
      collapsed: g.collapsed && !narrowed,
      apps: g.apps.filter((a) => hit(a.name) && FOUND_VIEWS[view](a)),
    }))
    .filter((g) => g.apps.length > 0);
  return {
    groups: view === 'largest' ? largestFirst(shown) : shown,
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

/** Why the selection can't be used yet, from the engine's view of it (plugins are asked apart). */
export const selectionIssues = (view: SelectionView): string[] =>
  view.issues.filter((i) => i.plugin === null).map((i) => i.problem);

/** Plugins that change files here before they leave and still wait for the user's answer. */
export function pluginAsks(view: SelectionView): PluginConsentModel[] {
  const waiting = new Set(view.issues.flatMap((i) => (i.plugin === null ? [] : [i.plugin])));
  const label = (key: string): string => view.rows.find((r) => r.key === key)?.facts?.label ?? key;
  return view.plugins
    .filter((p) => waiting.has(p.plugin))
    .map(({ plugin, version, description, permissions, items }) => ({
      plugin,
      version,
      description,
      permissions,
      items: items.map(label),
    }));
}

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

/** The Manual picks, as listed (`carrova-view` `PickRow`). */
export const manualPicks = (result: ScanResult): ManualPick[] =>
  result.view.picks.map((p) => ({
    key: p.key,
    path: p.display,
    kind: p.facts.kind,
    bytes: p.facts.size,
    files: p.facts.files,
    sensitive: p.tags.some((tag) => tag.kind === 'sensitive'),
    included: p.included,
  }));

/** The rail's facts in Manual mode: picks ticked, their bytes, sensitive ones brought (not keys). */
export function manualFacts(result: ScanResult): MoveFacts {
  const ticked = manualPicks(result).filter((p) => p.included);
  return {
    apps: 0,
    totalApps: 0,
    bytes: ticked.reduce((n, p) => n + p.bytes, 0),
    keys: 0,
    caches: false,
    picks: ticked.length,
    secrets: ticked.filter((p) => p.sensitive).length,
  };
}

/** A sensitive pick's confirm: the same dialog as a recipe secret. */
export const pickSecret = (pick: ManualPick): KeyConfirmModel => ({
  label: pick.path,
  why: 'secret',
  paths: [pick.path],
  keys: [],
});

/** `path` with home shown as `~`. */
const homeShown = (path: string, home: string | undefined): string =>
  home !== undefined && path.startsWith(`${home}/`) ? `~${path.slice(home.length)}` : path;

/** What the latest picks refused, and which were already inside a pick. */
export function pickNotes(
  outcomes: readonly PickOutcome[],
  result: ScanResult | null,
): { rejected: RejectedPick[]; absorbed: AbsorbedPick[] } {
  const home = result?.report.platform.roots.home;
  const shown = new Map(result?.view.picks.map((p) => [p.key, p.display]));
  return {
    rejected: outcomes.flatMap(({ path, result: r }) =>
      r.kind === 'rejected' ? [{ path: homeShown(path, home), reason: r.reason }] : [],
    ),
    absorbed: outcomes.flatMap(({ path, result: r }) =>
      r.kind === 'absorbed'
        ? [{ path: homeShown(path, home), into: shown.get(r.into) ?? r.into }]
        : [],
    ),
  };
}

/** Picked paths still being measured, as the user knows them. */
export const measuringPaths = (pending: readonly string[], result: ScanResult | null): string[] =>
  pending.map((path) => homeShown(path, result?.report.platform.roots.home));
