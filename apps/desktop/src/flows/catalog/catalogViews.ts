import type { TFunction } from 'i18next';
import type { EngineCatalogEntry, RecipesStatus } from '../../engine/types';
import type {
  CatalogDetail,
  CatalogEntry,
  CatalogGroup,
  CatalogModel,
  RecipeItem,
  TierFilter,
} from '../../screens/apps/model';
import { visibleEntries } from '../../screens/apps/visibleEntries';
import type { HubStatus } from '../../ui';
import { appRef } from '../shared/paths';

/** Rows a category shows before "Show all". */
const ROWS_SHOWN = 5;

/** UI-only state of the catalogue page. */
export interface CatalogUi {
  readonly filter: TierFilter;
  readonly query: string;
  /** The app open in the detail pane. */
  readonly selected: string | null;
  /** Categories listed in full. */
  readonly expanded: readonly string[];
}

const entry = (e: EngineCatalogEntry): CatalogEntry => ({
  ...appRef(e.id, e.name),
  tier: e.tier,
  here: e.detected,
});

function itemTag(item: EngineCatalogEntry['items'][number]): RecipeItem['tag'] {
  if (item.unmovable) return 'unmovable';
  if (item.sensitive) return 'sensitive';
  return item.default ? 'suggested' : null;
}

/** An app's items: suggested and sensitive ones move; the rest are left out. */
function detail(e: EngineCatalogEntry): CatalogDetail {
  const items = e.items.map((item) => ({ id: item.id, label: item.label, tag: itemTag(item) }));
  const moves = (item: RecipeItem) => item.tag === 'suggested' || item.tag === 'sensitive';
  return {
    ...entry(e),
    description: e.description,
    moves: items.filter(moves),
    leftOut: items.filter((item) => !moves(item)),
  };
}

/** Matching apps by category, in the order categories first appear. A search lists every match. */
function groups(entries: readonly EngineCatalogEntry[], ui: CatalogUi): CatalogGroup[] {
  const byCategory = new Map<string, EngineCatalogEntry[]>();
  for (const e of visibleEntries(entries, ui.filter, ui.query)) {
    byCategory.set(e.category, [...(byCategory.get(e.category) ?? []), e]);
  }
  const everything = ui.query.trim() !== '';
  return [...byCategory].map(([category, all]) => ({
    category,
    total: all.length,
    shown: (everything || ui.expanded.includes(category) ? all : all.slice(0, ROWS_SHOWN)).map(
      entry,
    ),
  }));
}

/** The catalogue list and detail, with the bundle's build time when it came signed. */
export function catalogModel(
  entries: readonly EngineCatalogEntry[],
  status: RecipesStatus | null,
  ui: CatalogUi,
  now: Date,
): CatalogModel {
  const created = status === null || status.origin.kind === 'dev_dir' ? null : status.created;
  const selected = entries.find((e) => e.id === ui.selected);
  return {
    now,
    filter: ui.filter,
    query: ui.query,
    groups: groups(entries, ui),
    selected: selected === undefined ? null : detail(selected),
    bundle: { updated: created === null ? null : new Date(created * 1000) },
  };
}

/** The sidebar foot: whether the recipes in use are signed, which list, how many apps. */
export function bundleStatus(status: RecipesStatus | null, t: TFunction): HubStatus | null {
  if (status === null) return null;
  const apps = t('count.apps', {
    count: status.verified + status.community + status.unverified,
  });
  return status.origin.kind === 'dev_dir'
    ? { title: t('apps.bundle.unsigned'), tone: 'warning', detail: t('apps.bundle.dev', { apps }) }
    : {
        title: t('apps.bundle.signed'),
        tone: 'success',
        detail: t('apps.bundle.list', { version: status.origin.version, apps }),
      };
}
