import { describe, expect, it } from 'vitest';
import type { EngineCatalogEntry, RecipesStatus } from '@/engine/types';
import { bundleStatus, catalogModel } from '@/flows/catalog/catalogViews';
import { i18n } from '@/i18n/i18n';

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
  ...['a', 'b', 'c', 'd', 'e', 'f'].map((id): EngineCatalogEntry => ({
    id,
    name: `Editor ${id}`,
    category: 'editors',
    description: '',
    tier: id === 'f' ? 'community' : 'verified',
    detected: id !== 'a',
    items: [{ id: 'x', label: 'Settings', default: false, sensitive: false, unmovable: false }],
  })),
];

const base = { filter: 'all', query: '', selected: null, expanded: [] } as const;

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
    const now = new Date();
    expect(catalogModel(entries, status('installed'), base, now).bundle.updated).toEqual(
      new Date(1_762_400_000_000),
    );
    expect(catalogModel(entries, status('dev_dir'), base, now).bundle.updated).toBeNull();
  });

  it('groups by category, five rows each, with the whole category counted', () => {
    const [ssh, editors] = catalogModel(entries, null, base, new Date()).groups;
    expect(ssh).toMatchObject({ category: 'dev-tools', total: 1 });
    expect(editors?.total).toBe(6);
    expect(editors?.shown.map((e) => e.id)).toEqual(['a', 'b', 'c', 'd', 'e']);
    expect(editors?.shown[0]?.here).toBe(false);
  });

  it('shows every row of an expanded category, and of any search', () => {
    const now = new Date();
    const open = catalogModel(entries, null, { ...base, expanded: ['editors'] }, now);
    expect(open.groups[1]?.shown).toHaveLength(6);
    const found = catalogModel(entries, null, { ...base, query: 'editor' }, now);
    expect(found.groups).toHaveLength(1);
    expect(found.groups[0]?.shown).toHaveLength(6);
  });

  it('filters by tier before counting', () => {
    const model = catalogModel(entries, null, { ...base, filter: 'community' }, new Date());
    expect(model.groups.map((g) => [g.category, g.total])).toEqual([['editors', 1]]);
  });

  it('splits the selected app into what moves and what is left out', () => {
    const { selected } = catalogModel(entries, null, { ...base, selected: 'ssh' }, new Date());
    expect(selected?.moves.map((i) => [i.id, i.tag])).toEqual([
      ['config', 'suggested'],
      ['keys', 'sensitive'],
    ]);
    expect(selected?.leftOut.map((i) => [i.id, i.tag])).toEqual([['agent', 'unmovable']]);
    expect(
      catalogModel(entries, null, { ...base, selected: 'a' }, new Date()).selected?.leftOut,
    ).toHaveLength(1);
    expect(
      catalogModel(entries, null, { ...base, selected: 'nope' }, new Date()).selected,
    ).toBeNull();
  });

  it('names the bundle in the sidebar foot, and says when recipes are not signed', () => {
    const t = i18n.t.bind(i18n);
    expect(bundleStatus(status('installed'), t)).toEqual({
      title: 'Signed recipes',
      tone: 'success',
      detail: 'Recipe list 3 · 1 app',
    });
    expect(bundleStatus(status('dev_dir'), t)).toEqual({
      title: 'Recipes not signed',
      tone: 'warning',
      detail: 'Development folder · 1 app',
    });
    expect(bundleStatus(null, t)).toBeNull();
  });
});
