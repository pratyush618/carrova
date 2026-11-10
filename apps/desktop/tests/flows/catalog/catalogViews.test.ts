import { describe, expect, it } from 'vitest';
import type { EngineCatalogEntry, RecipesStatus } from '@/engine/types';
import { catalogModel, recipeModel } from '@/flows/catalog/catalogViews';

const entries: EngineCatalogEntry[] = [
  {
    id: 'ssh',
    name: 'SSH',
    category: 'dev-tools',
    description: 'Keys and config',
    tier: 'verified',
    detected: true,
    items: [
      { id: 'config', label: 'Config', default: true, sensitive: false, unmovable: false },
      { id: 'keys', label: 'Keys', default: false, sensitive: true, unmovable: false },
      { id: 'agent', label: 'Agent', default: false, sensitive: false, unmovable: true },
    ],
  },
];

const status = (kind: 'installed' | 'dev_dir'): RecipesStatus => ({
  origin: kind === 'dev_dir' ? { kind } : { kind, version: 3 },
  created: kind === 'dev_dir' ? null : 1_762_400_000,
  verified: kind === 'dev_dir' ? 0 : 1,
  community: 0,
  unverified: kind === 'dev_dir' ? 1 : 0,
  skipped: [],
});

describe('catalogue views', () => {
  it('shows the bundle date only for signed bundles', () => {
    const ui = { filter: 'all', query: '' } as const;
    const now = new Date();
    expect(catalogModel(entries, status('installed'), ui, now).bundle.updated).toEqual(
      new Date(1_762_400_000_000),
    );
    expect(catalogModel(entries, status('dev_dir'), ui, now).bundle.updated).toBeNull();
  });

  it('tags recipe items: unmovable over sensitive over suggested', () => {
    expect(recipeModel(entries, 'ssh')?.items.map((i) => i.tag)).toEqual([
      'suggested',
      'sensitive',
      'unmovable',
    ]);
    expect(recipeModel(entries, 'nope')).toBeNull();
  });
});
