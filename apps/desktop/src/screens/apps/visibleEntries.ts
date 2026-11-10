import type { CatalogEntry, TierFilter } from './model';

/** Entries matching the tier filter and the search text (name, case-insensitive). */
export function visibleEntries(
  entries: readonly CatalogEntry[],
  filter: TierFilter,
  query: string,
): CatalogEntry[] {
  const needle = query.trim().toLocaleLowerCase();
  return entries.filter(
    (e) =>
      (filter === 'all' || e.tier === filter) &&
      (needle === '' || e.name.toLocaleLowerCase().includes(needle)),
  );
}
