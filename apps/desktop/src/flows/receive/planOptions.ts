import type {
  ConflictRule,
  ItemKey,
  MoveDefaults,
  OfferedItem,
  PlanOptions,
  Remap,
} from '../../engine/types';
import { ENGINE_DEFAULTS } from '../../bindings/constants';
import type { ImportChoices } from '../../screens/receive/model';
import { appOf } from '../shared/keys';

/**
 * The engine's suggested options (`ENGINE_DEFAULTS.plan_options`: every offered
 * item the receiver doesn't treat as sensitive, back up and replace on clashes,
 * nothing overridden) with the user's own suggested defaults (Settings) on top.
 */
export function suggestedPlanOptions(defaults: MoveDefaults): PlanOptions {
  return {
    ...ENGINE_DEFAULTS.plan_options,
    default_rule: defaults.conflict_rule,
    retention: defaults.retention,
  };
}

const without = <T>(list: readonly T[], value: T): T[] => list.filter((v) => v !== value);
const withOne = <T>(list: readonly T[], value: T): T[] => [...without(list, value), value];

/** The explicit item list behind `items: null` (picked offers), so it can be edited. */
const pickedKeys = (options: PlanOptions, offered: readonly OfferedItem[]): ItemKey[] =>
  options.items ?? offered.filter((o) => o.picked).map((o) => o.key);

/** The receiver's answer for a sensitive item: confirm it (and pick it) or leave it behind. */
export function answerSensitive(
  options: PlanOptions,
  offered: readonly OfferedItem[],
  key: ItemKey,
  bring: boolean,
): PlanOptions {
  const items =
    options.items === null
      ? null
      : bring
        ? withOne(options.items, key)
        : without(options.items, key);
  const confirmed = bring
    ? withOne(options.confirmed_sensitive, key)
    : without(options.confirmed_sensitive, key);
  return {
    ...options,
    items: items === null && !bring ? without(pickedKeys(options, offered), key) : items,
    confirmed_sensitive: confirmed,
  };
}

/** Leave every item of `app` out (it's open and the user chose to skip it), or bring them back. */
export function skipApp(
  options: PlanOptions,
  offered: readonly OfferedItem[],
  app: string,
  skip: boolean,
): PlanOptions {
  const ofApp = offered.filter((o) => appOf(o.key) === app);
  const base = pickedKeys(options, offered).filter((k) => appOf(k) !== app);
  const back = ofApp.filter(
    (o) => o.sensitive === null || options.confirmed_sensitive.includes(o.key),
  );
  return { ...options, items: skip ? base : [...base, ...back.map((o) => o.key)] };
}

/** Restore a same-system item from another system anyway (picked again if it was left). */
export function allowCrossOs(options: PlanOptions, item: ItemKey): PlanOptions {
  return {
    ...options,
    items: options.items === null ? null : withOne(options.items, item),
    allow_cross_os: withOne(options.allow_cross_os, item),
  };
}

/** Let a plugin change the files of the items that use it (the user saw its permissions). */
export function allowPlugin(options: PlanOptions, plugin: string): PlanOptions {
  return { ...options, confirmed_plugins: withOne(options.confirmed_plugins, plugin) };
}

/** Clash rule for a whole item. */
export function setItemRule(options: PlanOptions, item: ItemKey, rule: ConflictRule): PlanOptions {
  return { ...options, item_rules: { ...options.item_rules, [item]: rule } };
}

/** Clash rule for one target path (`root:rel`); `null` follows the item's rule again. */
export function setPathRule(
  options: PlanOptions,
  path: string,
  rule: ConflictRule | null,
): PlanOptions {
  const rest = Object.fromEntries(Object.entries(options.path_rules).filter(([p]) => p !== path));
  return { ...options, path_rules: rule === null ? rest : { ...rest, [path]: rule } };
}

/** Restore an item elsewhere (re-confined by the engine); `null` puts it back where it was. */
export function setRemap(options: PlanOptions, item: ItemKey, remap: Remap | null): PlanOptions {
  const rest = Object.fromEntries(Object.entries(options.remaps).filter(([k]) => k !== item));
  return { ...options, remaps: remap === null ? rest : { ...rest, [item]: remap } };
}

/** Import-wide choices from the Every change sheet (an empty suffix keeps the suggestion). */
export function setChoices(options: PlanOptions, patch: Partial<ImportChoices>): PlanOptions {
  const suffix = patch.keepBothSuffix?.trim();
  return {
    ...options,
    keep_both_suffix: suffix === undefined || suffix === '' ? options.keep_both_suffix : suffix,
    retention: patch.retention ?? options.retention,
    import_trust: patch.importTrust ?? options.import_trust,
    keep_group_write: patch.keepGroupWrite ?? options.keep_group_write,
  };
}

/** Plan-level overrides, each confirmed in the override dialog first. */
export type PlanOverride = 'running' | 'space';

export function applyOverride(options: PlanOptions, override: PlanOverride): PlanOptions {
  return override === 'running'
    ? { ...options, ignore_requires_closed: true }
    : { ...options, ignore_free_space: true };
}
