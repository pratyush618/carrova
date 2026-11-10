/**
 * The fake port's planner: re-plans a recorded review under the user's
 * `PlanOptions`, for the fields the review screens edit — item selection,
 * sensitive confirmations, conflict rules (default, per item, per file),
 * remaps and the requires-closed override. Mirrors the engine's rules
 * (`PlanOptions::picks`, rule precedence path > item > default); it does
 * not re-run the engine's checks, nor re-judge the sensitivity of a remapped
 * item at its new target. `tests/ipc/fake/planner.test.ts` holds it to plans
 * the engine recorded for the same options.
 */
import type { Action } from '../../bindings/Action';
import type { ActionCounts } from '../../bindings/ActionCounts';
import type { Blocker } from '../../bindings/Blocker';
import type { ConflictRule } from '../../bindings/ConflictRule';
import type { ItemView } from '../../bindings/ItemView';
import type { OfferedItem } from '../../bindings/OfferedItem';
import type { PlannedAction } from '../../bindings/PlannedAction';
import type { PlanOptions } from '../../bindings/PlanOptions';
import type { PlanReview } from '../../bindings/PlanReview';

/** Extra blockers of the scenario (e.g. a running app), unless overridden. */
export interface PlanConditions {
  readonly runningApp: Blocker | null;
}

/** `PlanOptions::picks`: an explicit list wins; else all but unconfirmed sensitive items. */
function picks(options: PlanOptions, item: OfferedItem): boolean {
  if (options.items !== null) return options.items.includes(item.key);
  return item.sensitive === null || options.confirmed_sensitive.includes(item.key);
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

/** `base` (a review planned with every sensitive item confirmed) re-planned under `options`. */
export function planUnder(
  base: PlanReview,
  options: PlanOptions,
  conditions: PlanConditions,
): PlanReview {
  const offered = base.offered.map((o) => ({
    ...o,
    picked: picks(options, o),
    sensitive:
      o.sensitive === null
        ? null
        : { ...o.sensitive, confirmed: options.confirmed_sensitive.includes(o.key) },
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
  const items = base.view.items.filter((i) => picked.has(i.key)).map((i) => itemView(i, actions));
  return {
    ...base,
    offered,
    plan: {
      ...base.plan,
      options,
      actions,
      blockers,
      bytes_to_write: actions.filter(writes).reduce((n, a) => n + a.size, 0),
    },
    view: { ...base.view, items },
  };
}
