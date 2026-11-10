import type { AppRef } from '../../screens/shared/model';
import type { SemanticRoot } from '../../engine/types';

/** Where each semantic root resolved on this computer (from the scan's platform report). */
export type RootDirs = Readonly<Partial<Record<SemanticRoot, string>>>;

const ROOTS: readonly SemanticRoot[] = ['user_config', 'user_data', 'cache', 'home', 'documents'];

/** Splits an engine path key `root:rel`; `null` when the root is unknown. */
export function splitRooted(key: string): { root: SemanticRoot; rel: string } | null {
  const colon = key.indexOf(':');
  const root = ROOTS.find((r) => r === key.slice(0, colon));
  return root === undefined ? null : { root, rel: key.slice(colon + 1) };
}

/**
 * A path key as the user knows it: the root's real directory (home shown as
 * `~`) joined with the relative path. Falls back to the key itself.
 */
export function displayPath(key: string, dirs: RootDirs): string {
  const parts = splitRooted(key);
  if (parts === null) return key;
  const dir = dirs[parts.root];
  if (dir === undefined) return key;
  const home = dirs.home;
  const shown = home !== undefined && dir.startsWith(home) ? `~${dir.slice(home.length)}` : dir;
  return parts.rel === '' ? shown : `${shown}/${parts.rel}`;
}

/** An app's monogram and name. Recipes carry no icon, so the glyph is its id's first letters. */
export function appRef(id: string, name: string): AppRef {
  return { id, name, glyph: id.slice(0, 3) };
}
