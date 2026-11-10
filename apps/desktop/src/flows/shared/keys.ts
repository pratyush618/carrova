import type { ItemKey } from '../../engine/types';

/** The app part of a recipe item key (`git/config` → `git`); raw keys have none. */
export function appOf(key: ItemKey): string | null {
  if (key.startsWith('raw:')) return null;
  const slash = key.indexOf('/');
  return slash < 0 ? null : key.slice(0, slash);
}
