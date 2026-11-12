import type { Tier, TierFilter } from './model';

/** Entries matching the tier filter and the search text (name, case-insensitive). */
export function visibleEntries<E extends { readonly name: string; readonly tier: Tier }>(
  entries: readonly E[],
  filter: TierFilter,
  query: string,
): E[] {
  const needle = query.trim().toLocaleLowerCase();
  return entries.filter(
    (e) =>
      (filter === 'all' || e.tier === filter) &&
      (needle === '' || e.name.toLocaleLowerCase().includes(needle)),
  );
}
