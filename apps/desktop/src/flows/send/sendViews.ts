import type { TFunction } from 'i18next';
import type { ItemRoles } from '../../bindings/ItemRoles';
import type { SelectionRow } from '../../bindings/SelectionRow';
import type { ItemChoice, ScanResult, SelectionView } from '../../engine/types';
import type {
  CustomizeModel,
  FoundApp,
  ItemTag,
  RawFolder,
  SensitiveAsk,
  TreeApp,
  TreeItem,
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

const detected = (result: ScanResult): AppReport[] =>
  result.report.apps.filter((a) => a.detected && a.items.length > 0);

/** The one tag shown next to an item: the most important of the engine's marks. */
export function tagOf(tags: SelectionRow['tags']): ItemTag | null {
  const order: readonly ItemTag['kind'][] = ['unmovable', 'sensitive', 'suggested'];
  const ranked = [...tags].sort((a, b) => order.indexOf(a.kind) - order.indexOf(b.kind));
  return ranked[0] ?? null;
}

/** The summary rows of "Here's what we found": one per detected app. */
export function foundApps(result: ScanResult): FoundApp[] {
  return detected(result).map((app) => {
    const ordinary = entries(app, result).filter((e) => e.roles.ordinary);
    return {
      ...appRef(app.id, app.name),
      what: ordinary.map((e) => e.item.label).join(', '),
      files: ordinary.reduce((n, e) => n + e.item.file_count, 0),
      bytes: ordinary.reduce((n, e) => n + e.item.size, 0),
      included: ordinary.some((e) => e.choice.included),
    };
  });
}

/** Sensitive items, asked once in plain words; they start on "Leave behind". */
export function sensitiveAsks(result: ScanResult): SensitiveAsk[] {
  return detected(result).flatMap((app) =>
    entries(app, result)
      .filter((e) => e.choice.sensitive && e.roles.movable)
      .map((e) => ({
        key: e.key,
        title: `${app.name}: ${e.item.label}`,
        why: e.item.sensitive_reason,
        kind: 'secret' as const,
        answer: e.choice.included && e.choice.confirmed_sensitive ? 'bring' : 'leave',
      })),
  );
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
  const caches = flat.filter((e) => e.roles.cache && !e.choice.sensitive);
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
    includeCaches: caches.length > 0 && caches.every((e) => e.choice.included),
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
      const key = `raw:${r.root}/${r.rel}`;
      return {
        key,
        path: displayPath(`${r.root}:${r.rel}`, dirs),
        hint: t('count.files', { count: r.file_count }),
        bytes: r.size,
        picked: result.selection.items[key]?.included ?? false,
      };
    });
}
