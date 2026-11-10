import type { ItemKey, Selection } from '../../engine/types';
import type { Preset } from '../../bindings/Preset';
import type { SelectionEdit } from '../../bindings/SelectionEdit';

/**
 * One change the user makes on screen. The engine applies it
 * (`Selection::apply_all`); this module only spells it as the engine's edits.
 */
export type SelectionChange =
  /** Start over from a preset. Keys are asked one by one, so it never touches them. */
  | { readonly kind: 'preset'; readonly preset: Preset }
  /** The app's row: ticks or unticks its ordinary items. */
  | { readonly kind: 'app'; readonly app: string; readonly included: boolean }
  /** One item in Customize. Ticking a sensitive item there is its explicit confirmation. */
  | { readonly kind: 'item'; readonly key: ItemKey; readonly included: boolean }
  /** One declared path under an item. */
  | {
      readonly kind: 'path';
      readonly key: ItemKey;
      readonly path: string;
      readonly included: boolean;
    }
  /** The "Needs your OK" answer for a sensitive item. */
  | { readonly kind: 'sensitive'; readonly key: ItemKey; readonly bring: boolean }
  /** Include an unmovable item anyway, after the override dialog. */
  | { readonly kind: 'force'; readonly key: ItemKey }
  /** Every movable, non-sensitive item under the cache root. */
  | { readonly kind: 'caches'; readonly included: boolean };

/** `change` as the engine's edits of `selection`. */
export function toEdits(change: SelectionChange, selection: Selection): SelectionEdit[] {
  switch (change.kind) {
    case 'app':
    case 'caches':
      return [change];
    case 'item': {
      const tick: SelectionEdit = { kind: 'item', item: change.key, included: change.included };
      return selection.items[change.key]?.sensitive === true
        ? [tick, { kind: 'confirm_sensitive', item: change.key, confirmed: change.included }]
        : [tick];
    }
    case 'preset':
      // The engine's preset unticks every key; re-tick the ones already confirmed one by one.
      return [
        change,
        ...Object.entries(selection.items)
          .filter(([, c]) => c.sensitive && c.included && c.confirmed_sensitive)
          .map(([item]): SelectionEdit => ({ kind: 'item', item, included: true })),
      ];
    case 'path':
      return [{ kind: 'path', item: change.key, path: change.path, included: change.included }];
    case 'sensitive':
      return [
        { kind: 'confirm_sensitive', item: change.key, confirmed: change.bring },
        { kind: 'item', item: change.key, included: change.bring },
      ];
    case 'force':
      return [
        { kind: 'force_unmovable', item: change.key },
        { kind: 'item', item: change.key, included: true },
      ];
  }
}
