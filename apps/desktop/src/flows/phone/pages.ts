import type { Cursor } from '../../bindings/Cursor';

/**
 * The newest `pages` pages from `load`, each fetched from the cursor the
 * one before it ended at and joined by `join(newer, older)` (which keeps
 * `older.next`). Loaded again whole on every refresh: kept older pages
 * would leave a gap once new rows push the first page along.
 */
export async function loadPages<V extends { readonly next: Cursor | null }>(
  load: (before: Cursor | null) => Promise<V>,
  pages: number,
  join: (newer: V, older: V) => V,
): Promise<V> {
  let view = await load(null);
  for (let page = 1; page < pages && view.next !== null; page++)
    view = join(view, await load(view.next));
  return view;
}
