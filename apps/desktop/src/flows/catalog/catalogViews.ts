import type { EngineCatalogEntry, RecipesStatus } from '../../engine/types';
import type {
  CatalogEntry,
  CatalogModel,
  RecipeItem,
  RecipeModel,
  TierFilter,
} from '../../screens/apps/model';
import { appRef } from '../shared/paths';

/** UI-only state of the catalogue page. */
export interface CatalogUi {
  readonly filter: TierFilter;
  readonly query: string;
}

const entry = (e: EngineCatalogEntry): CatalogEntry => ({
  ...appRef(e.id, e.name),
  tier: e.tier,
  installed: e.detected,
});

/** The catalogue grid, with the bundle's build time when it came signed. */
export function catalogModel(
  entries: readonly EngineCatalogEntry[],
  status: RecipesStatus | null,
  ui: CatalogUi,
  now: Date,
): CatalogModel {
  const created = status === null || status.origin.kind === 'dev_dir' ? null : status.created;
  return {
    now,
    filter: ui.filter,
    query: ui.query,
    entries: entries.map(entry),
    bundle: { updated: created === null ? null : new Date(created * 1000) },
  };
}

function itemTag(item: EngineCatalogEntry['items'][number]): RecipeItem['tag'] {
  if (item.unmovable) return 'unmovable';
  if (item.sensitive) return 'sensitive';
  return item.default ? 'suggested' : null;
}

/** One app's detail sheet; `null` for an id the catalogue doesn't have. */
export function recipeModel(
  entries: readonly EngineCatalogEntry[],
  id: string,
): RecipeModel | null {
  const e = entries.find((x) => x.id === id);
  if (e === undefined) return null;
  return {
    ...appRef(e.id, e.name),
    description: e.description,
    tier: e.tier,
    installed: e.detected,
    items: e.items.map((item) => ({ id: item.id, label: item.label, tag: itemTag(item) })),
  };
}
