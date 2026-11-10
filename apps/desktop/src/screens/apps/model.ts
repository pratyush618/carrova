import type { AppRef } from '../shared/model';

/** Recipe trust tier (`recipes/TIERS.yaml`); dev recipes are unverified. */
export type Tier = 'verified' | 'community' | 'unverified';
export type TierFilter = 'all' | 'verified' | 'community';

/** One recipe in the catalogue. */
export interface CatalogEntry extends AppRef {
  readonly tier: Tier;
  /** Detected on this computer. */
  readonly installed: boolean;
}

export interface CatalogModel {
  readonly now: Date;
  readonly filter: TierFilter;
  readonly query: string;
  readonly entries: readonly CatalogEntry[];
  /** When the recipe bundle was built; `null` for unsigned development recipes. */
  readonly bundle: { readonly updated: Date | null };
}

export interface CatalogIntents {
  readonly onFilter: (filter: TierFilter) => void;
  readonly onQuery: (query: string) => void;
  readonly onOpen: (id: string) => void;
}

export interface RecipeItem {
  readonly id: string;
  readonly label: string;
  readonly tag: 'suggested' | 'sensitive' | 'unmovable' | null;
}

export interface RecipeModel extends AppRef {
  readonly description: string;
  readonly tier: Tier;
  readonly installed: boolean;
  readonly items: readonly RecipeItem[];
}

export interface RecipeIntents {
  readonly onClose: () => void;
}
