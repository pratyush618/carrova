import type { AppRef } from '../shared/model';

/** Recipe trust tier (`recipes/TIERS.yaml`); dev recipes are unverified. */
export type Tier = 'verified' | 'community' | 'unverified';
export type TierFilter = 'all' | 'verified' | 'community';

/** One recipe in the catalogue. */
export interface CatalogEntry extends AppRef {
  readonly tier: Tier;
  /** Detected on this computer. */
  readonly here: boolean;
}

/** One category of the list: its first rows, and how many apps it holds in all. */
export interface CatalogGroup {
  /** Recipe category id (`dev-tools`). */
  readonly category: string;
  readonly total: number;
  readonly shown: readonly CatalogEntry[];
}

export interface RecipeItem {
  readonly id: string;
  readonly label: string;
  readonly tag: 'suggested' | 'sensitive' | 'unmovable' | null;
}

/** The selected app: what its recipe moves and what it leaves out. */
export interface CatalogDetail extends CatalogEntry {
  readonly description: string;
  readonly moves: readonly RecipeItem[];
  readonly leftOut: readonly RecipeItem[];
}

export interface CatalogModel {
  readonly now: Date;
  readonly filter: TierFilter;
  readonly query: string;
  readonly groups: readonly CatalogGroup[];
  readonly selected: CatalogDetail | null;
  /** When the recipe bundle was built; `null` for unsigned development recipes. */
  readonly bundle: { readonly updated: Date | null };
}

export interface CatalogIntents {
  readonly onFilter: (filter: TierFilter) => void;
  readonly onQuery: (query: string) => void;
  readonly onSelect: (id: string) => void;
  readonly onShowAll: (category: string) => void;
}
