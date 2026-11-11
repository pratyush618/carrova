/**
 * The fake port's planner: re-plans a recorded review under the user's
 * `PlanOptions`, for the fields the review screens edit — item selection,
 * sensitive confirmations, conflict rules (default, per item, per file),
 * remaps, the requires-closed override and Accept all (safe picks). Mirrors the engine's rules
 * (`PlanOptions::picks`, rule precedence path > item > default); it does
 * not re-run the engine's checks, but does re-judge a remapped item at its
 * new targets (`rejudged`). `tests/ipc/fake/planner.test.ts` holds it to plans
 * the engine recorded for the same options. `editOptions` applies plan
 * edits by the engine's rules (`PlanOptions::apply_all`).
 */
import type { Action } from '../../bindings/Action';
import type { ActionCounts } from '../../bindings/ActionCounts';
import type { Blocker } from '../../bindings/Blocker';
import type { ConflictRule } from '../../bindings/ConflictRule';
import type { ItemKey } from '../../bindings/ItemKey';
import type { ItemView } from '../../bindings/ItemView';
import type { OfferedItem } from '../../bindings/OfferedItem';
import type { OsKind } from '../../bindings/OsKind';
import type { PathMapping } from '../../bindings/PathMapping';
import type { PlannedAction } from '../../bindings/PlannedAction';
import type { PlannedPlugin } from '../../bindings/PlannedPlugin';
import type { PlanEdit } from '../../bindings/PlanEdit';
import type { PlanOptions } from '../../bindings/PlanOptions';
import type { PlanReview } from '../../bindings/PlanReview';
import type { SemanticRoot } from '../../bindings/SemanticRoot';
import { bridgeError, raise } from '../reject';

/** An archive from another system: its OS, its same-system items, and where paths land. */
export interface CrossOsConditions {
  readonly source: OsKind;
  /** Items whose recipe keeps them on their system (each a `cross_os` blocker unless allowed). */
  readonly sameOs: readonly ItemKey[];
  readonly mappings: readonly PathMapping[];
}

/** Extra conditions of the scenario (a running app, another system, a plugin), unless overridden. */
export interface PlanConditions {
  readonly runningApp: Blocker | null;
  readonly crossOs?: CrossOsConditions;
  /** A plugin run over its items' files (an `unconfirmed_plugin` blocker until allowed). */
  readonly plugin?: PlannedPlugin;
}

/**
 * `PlanOptions::confirms`: confirmed by the user, or a safe pick accepted
 * with Accept all that the user did not leave.
 */
const confirmed = (options: PlanOptions, item: OfferedItem): boolean =>
  options.confirmed_sensitive.includes(item.key) ||
  (options.accept_safe_picks && item.safe_pick && !options.declined.includes(item.key));

/**
 * `PlanOptions::picks`: an explicit list wins, else all but unconfirmed
 * sensitive items.
 */
function picks(options: PlanOptions, item: OfferedItem): boolean {
  if (options.items !== null) return options.items.includes(item.key);
  return item.sensitive === null || confirmed(options, item);
}

/** The `root:rel` text per-file rules are keyed by. */
const targetKey = (a: PlannedAction): string => `${a.root}:${a.rel}`;

/** Whether `a` writes over (or next to) something that exists. */
const isClash = (a: PlannedAction): boolean =>
  a.action.action === 'replace' || a.action.action === 'keep-both';

function ruleFor(options: PlanOptions, a: PlannedAction): ConflictRule {
  return options.path_rules[targetKey(a)] ?? options.item_rules[a.item] ?? options.default_rule;
}

function actionFor(rule: ConflictRule, a: PlannedAction, suffix: string): Action {
  switch (rule) {
    case 'replace':
      return { action: 'replace' };
    case 'keep-both':
      return { action: 'keep-both', rel: `${a.rel}.${suffix}` };
    case 'skip':
      return { action: 'skip', reason: 'rule: keep the existing file' };
  }
}

/** `a` with the user's rule (if it clashes) and remap applied. */
function decide(options: PlanOptions, a: PlannedAction): PlannedAction {
  const suffix = options.keep_both_suffix.replace('{date}', '20261108');
  const action = isClash(a) ? actionFor(ruleFor(options, a), a, suffix) : a.action;
  const remap = options.remaps[a.item];
  if (remap === undefined) return { ...a, action };
  const prefix = remap.rel_prefix === null ? '' : `${remap.rel_prefix}/`;
  return { ...a, action, root: remap.root ?? a.root, rel: `${prefix}${a.rel}` };
}

const writes = (a: PlannedAction): boolean =>
  a.action.action === 'create' || a.action.action === 'replace' || a.action.action === 'keep-both';

function countKey(action: Action): keyof ActionCounts {
  switch (action.action) {
    case 'keep-both':
      return 'keep_both';
    case 'create':
    case 'replace':
    case 'skip':
    case 'identical':
      return action.action;
  }
}

function countsOf(actions: readonly PlannedAction[]): ItemView['counts'] {
  const counts: ActionCounts = { create: 0, replace: 0, keep_both: 0, skip: 0, identical: 0 };
  for (const a of actions) counts[countKey(a.action)] += 1;
  const bytes = actions.filter(writes).reduce((n, a) => n + a.size, 0);
  return { actions: counts, bytes };
}

function itemView(base: ItemView, actions: readonly PlannedAction[]): ItemView {
  const mine = actions.filter((a) => a.item === base.key);
  const lines = base.lines.map((line) => {
    if (line.line !== 'action') return line;
    const now = mine.find((a) => a.entry_index === line.entry_index);
    return now === undefined ? line : { line: 'action' as const, ...now };
  });
  return { key: base.key, counts: countsOf(mine), lines };
}

/** The engine's blocker for a picked but unconfirmed sensitive item. */
function unconfirmed(item: OfferedItem): Blocker {
  return { kind: 'unconfirmed_sensitive', item: item.key, reason: item.sensitive?.reason ?? '' };
}

/**
 * Well-known secret places, matched on any path component ignoring case
 * (the engine's `SECRET_NAMES`). shortcut: the most common rows only,
 * extend when a fake scenario remaps into another.
 */
const SECRET_NAMES: readonly (readonly [RegExp, string])[] = [
  [/^\.ssh$/, 'SSH keys'],
  [/^\.gnupg$/, 'GnuPG keys'],
  [/^(\.aws|\.azure|gcloud)$/, 'cloud credentials'],
  [/^\.kube$/, 'cluster credentials'],
  [/^id_(?!.*\.pub$)/, 'a private key'],
  [/\.pem$/, 'a key or certificate'],
  [/\.key$/, 'a private key'],
  [/^\.env(\..*)?$/, 'environment secrets'],
  [/^\.netrc$/, 'saved passwords'],
  [/^\.git-credentials$/, 'saved git passwords'],
];

/**
 * Places where incoming data can run code or grant access, matched on the
 * path and below (the engine's `DANGEROUS_PATHS`). shortcut: the common
 * rows only, extend with the engine's table when a scenario needs more.
 */
const DANGEROUS_PATHS: readonly (readonly [SemanticRoot, string, string])[] = [
  ['home', '.ssh', 'controls SSH logins, keys and commands'],
  ['home', '.gnupg', 'holds GnuPG keys, trust and agent programs'],
  ['home', '.bashrc', 'can run code at login'],
  ['home', '.profile', 'can run code at login'],
  ['home', '.zshrc', 'can run code at login'],
  ['user_config', 'autostart', 'starts programs at login (autostart)'],
  ['user_config', 'systemd/user', 'starts services at login (systemd user units)'],
  ['home', '.local/bin', 'puts programs on the command search path'],
  ['home', '.gitconfig', 'can make git run commands (core.sshCommand, hooks)'],
];

const within = (rel: string, base: string): boolean => rel === base || rel.startsWith(`${base}/`);

/** Why a final target `root:rel` of item `key` needs confirming, as the engine words it. */
function reasonsAt(key: ItemKey, root: SemanticRoot, rel: string): string[] {
  const at = `${root}:${rel}`;
  const names = rel.toLowerCase().split('/');
  // Secret names judge recipe-less (raw) items only, like the engine.
  const secret = key.startsWith('raw:')
    ? SECRET_NAMES.find(([name]) => names.some((n) => name.test(n)))
    : undefined;
  const dangers = DANGEROUS_PATHS.filter(
    ([r, base]) =>
      (r === root && within(rel, base)) ||
      (r === 'user_config' && root === 'home' && within(rel, `.config/${base}`)),
  );
  return [
    ...(secret === undefined ? [] : [`${secret[1]} (${at})`]),
    ...dangers.map(([, , why]) => `${why} (${at})`),
  ];
}

/**
 * `item` judged again at its remapped targets, like the engine: a place
 * that holds secrets or runs code makes it sensitive and no longer safe.
 */
function rejudged(item: OfferedItem, targets: readonly PlannedAction[]): OfferedItem {
  const found = targets.flatMap((a) => reasonsAt(item.key, a.root, a.rel));
  const reasons = [...new Set(found)];
  if (reasons.length === 0) return item;
  const before = item.sensitive?.reason;
  const reason = [...(before === undefined ? [] : [before]), ...reasons].join('; ');
  return { ...item, safe_pick: false, sensitive: { confirmed: false, reason } };
}

/** `base` (a review planned with every sensitive item confirmed) re-planned under `options`. */
export function planUnder(
  base: PlanReview,
  options: PlanOptions,
  conditions: PlanConditions,
): PlanReview {
  const judged = base.offered.map((o) =>
    o.key in options.remaps
      ? rejudged(
          o,
          base.plan.actions.filter((a) => a.item === o.key).map((a) => decide(options, a)),
        )
      : o,
  );
  const offered = judged.map((o) => ({
    ...o,
    picked: picks(options, o),
    sensitive: o.sensitive === null ? null : { ...o.sensitive, confirmed: confirmed(options, o) },
  }));
  const picked = new Set(offered.filter((o) => o.picked).map((o) => o.key));
  const actions = base.plan.actions
    .filter((a) => picked.has(a.item))
    .map((a) => decide(options, a));
  const blockers: Blocker[] = offered
    .filter((o) => o.picked && o.sensitive !== null && !o.sensitive.confirmed)
    .map(unconfirmed);
  if (conditions.runningApp !== null && !options.ignore_requires_closed) {
    blockers.push(conditions.runningApp);
  }
  const { crossOs, plugin } = conditions;
  // The fake receiver runs Linux (its About), like the machine that recorded `base`.
  const to: OsKind = 'linux';
  if (crossOs !== undefined) {
    for (const item of crossOs.sameOs) {
      if (picked.has(item) && !options.allow_cross_os.includes(item)) {
        blockers.push({ kind: 'cross_os', item, from: crossOs.source, to });
      }
    }
  }
  const plugins = plugin?.items.some((k) => picked.has(k)) ? [plugin] : [];
  for (const p of plugins) {
    if (!options.confirmed_plugins.includes(p.plugin)) {
      blockers.push({
        kind: 'unconfirmed_plugin',
        plugin: p.plugin,
        items: p.items.filter((k) => picked.has(k)),
      });
    }
  }
  const items = base.view.items.filter((i) => picked.has(i.key)).map((i) => itemView(i, actions));
  return {
    ...base,
    offered,
    source:
      crossOs === undefined
        ? base.source
        : { os: crossOs.source, device_name: base.source?.device_name ?? null },
    plan: {
      ...base.plan,
      options,
      actions,
      blockers,
      safe_picks: base.plan.safe_picks.filter((k) =>
        offered.some((o) => o.key === k && o.safe_pick),
      ),
      sensitive: {
        ...base.plan.sensitive,
        ...Object.fromEntries(
          offered.flatMap((o) => (o.sensitive === null ? [] : [[o.key, o.sensitive.reason]])),
        ),
      },
      mappings: crossOs?.mappings.filter((m) => picked.has(m.item)) ?? base.plan.mappings,
      plugins,
      bytes_to_write: actions.filter(writes).reduce((n, a) => n + a.size, 0),
    },
    view: { ...base.view, items },
  };
}

const without = <T>(list: readonly T[], value: T): T[] => list.filter((v) => v !== value);
/** Add `value` to `list` once (`on`) or remove it — the engine's set toggle. */
const toggle = <T>(list: readonly T[], value: T, on: boolean): T[] =>
  !on ? without(list, value) : list.includes(value) ? [...list] : [...list, value];
const withoutKey = <V>(map: Partial<Record<string, V>>, key: string) =>
  Object.fromEntries(Object.entries(map).filter(([k]) => k !== key)) as Record<string, V>;

/** The fake's stand-in for an item's recipe id: the key before its `/`. */
const appOf = (key: ItemKey): string => key.slice(0, key.indexOf('/'));

/** `PlanOptions::picked`: the offered items `options` picks, as an explicit list. */
const pickedKeys = (options: PlanOptions, offered: readonly OfferedItem[]): ItemKey[] =>
  offered.filter((o) => picks(options, o)).map((o) => o.key);

/** `PlanOptions::apply`: one edit against `review`'s offered items and plugins. */
function applyEdit(options: PlanOptions, edit: PlanEdit, review: PlanReview): PlanOptions {
  const offered = review.offered;
  const known = (item: ItemKey) => {
    if (!offered.some((o) => o.key === item)) {
      raise(bridgeError('invalid_selection', `unknown item \`${item}\``));
    }
  };
  switch (edit.kind) {
    case 'answer_sensitive': {
      known(edit.item);
      const list =
        options.items === null && !edit.bring ? pickedKeys(options, offered) : options.items;
      return {
        ...options,
        items: list === null ? null : toggle(list, edit.item, edit.bring),
        confirmed_sensitive: toggle(options.confirmed_sensitive, edit.item, edit.bring),
        declined: toggle(options.declined, edit.item, !edit.bring),
      };
    }
    case 'skip_app': {
      const base = pickedKeys(options, offered).filter((k) => appOf(k) !== edit.app);
      const back = offered
        .filter((o) => appOf(o.key) === edit.app)
        .filter((o) => o.sensitive === null || confirmed(options, o))
        .map((o) => o.key);
      return { ...options, items: edit.skip ? base : [...base, ...back] };
    }
    case 'allow_cross_os':
      known(edit.item);
      return {
        ...options,
        items:
          edit.allow && options.items !== null
            ? toggle(options.items, edit.item, true)
            : options.items,
        allow_cross_os: toggle(options.allow_cross_os, edit.item, edit.allow),
      };
    case 'allow_plugin': {
      const inUse = review.plan.plugins.some((p) => p.plugin === edit.plugin);
      if (!inUse && !options.confirmed_plugins.includes(edit.plugin)) {
        raise(bridgeError('invalid_selection', `unknown plugin \`${edit.plugin}\``));
      }
      return {
        ...options,
        confirmed_plugins: toggle(options.confirmed_plugins, edit.plugin, edit.allow),
      };
    }
    case 'item_rule':
      known(edit.item);
      return { ...options, item_rules: { ...options.item_rules, [edit.item]: edit.rule } };
    case 'path_rule': {
      const rest = withoutKey(options.path_rules, edit.path);
      return {
        ...options,
        path_rules: edit.rule === null ? rest : { ...rest, [edit.path]: edit.rule },
      };
    }
    case 'remap': {
      known(edit.item);
      const now = options.remaps[edit.item] ?? null;
      const same =
        now === edit.remap ||
        (now !== null &&
          edit.remap !== null &&
          now.root === edit.remap.root &&
          now.rel_prefix === edit.remap.rel_prefix);
      // `PlanOptions::remap`: a confirmation covers the place it was given for.
      if (same) return options;
      const rest = withoutKey(options.remaps, edit.item);
      return {
        ...options,
        confirmed_sensitive: without(options.confirmed_sensitive, edit.item),
        remaps: edit.remap === null ? rest : { ...rest, [edit.item]: edit.remap },
      };
    }
    case 'keep_both_suffix': {
      const suffix = edit.suffix.trim();
      return { ...options, keep_both_suffix: suffix === '' ? options.keep_both_suffix : suffix };
    }
    case 'retention':
      return { ...options, retention: edit.retention };
    case 'import_trust':
      return { ...options, import_trust: edit.allow };
    case 'keep_group_write':
      return { ...options, keep_group_write: edit.keep };
    case 'override':
      return edit.check === 'running'
        ? { ...options, ignore_requires_closed: true }
        : { ...options, ignore_free_space: true };
    case 'accept_safe_picks': {
      // Ticking is harmless: each plan confirms only its own safe picks.
      const safe = offered
        .filter((o) => o.safe_pick && !options.declined.includes(o.key))
        .map((o) => o.key);
      const items =
        options.items === null
          ? null
          : safe.reduce((list, k) => toggle(list, k, true), options.items);
      return { ...options, items, accept_safe_picks: true };
    }
  }
}

/**
 * `PlanOptions::apply_all`: `review`'s options with `edits` applied in order;
 * an unknown item or plugin raises `invalid_selection`.
 */
export function editOptions(review: PlanReview, edits: readonly PlanEdit[]): PlanOptions {
  return edits.reduce((options, edit) => applyEdit(options, edit, review), review.plan.options);
}
