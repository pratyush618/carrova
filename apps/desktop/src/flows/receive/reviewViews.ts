import type { PlannedPlugin } from '../../bindings/PlannedPlugin';
import type { Checklist, ItemKey, OsKind, PlanReview } from '../../engine/types';
import type {
  ArrivedModel,
  ChangeGroup,
  DecisionAnswer,
  DecisionModel,
  PluginConsentModel,
  ReviewDetailsModel,
  ReviewModel,
} from '../../screens/receive/model';
import type { Computer, MoveRoute } from '../../screens/shared/model';
import type { Remote } from '../remote';
import { appOf } from '../shared/keys';
import { appRef } from '../shared/paths';

/** The user's answers to review decisions: item keys (secrets) and app ids (open apps). */
export type Answers = Readonly<Record<string, DecisionAnswer>>;

type Action = PlanReview['plan']['actions'][number];

const isClash = (a: Action): boolean =>
  a.action.action === 'replace' || a.action.action === 'keep-both' || a.action.action === 'skip';

const targetKey = (a: Action): string => `${a.root}:${a.rel}`;

/** Totals over the whole plan: files added, replaced (backed up first) and already the same. */
export function planCounts(review: PlanReview): Omit<ReviewModel['counts'], 'left'> {
  return review.view.items.reduce(
    (sum, item) => ({
      added: sum.added + item.counts.actions.create,
      replaced: sum.replaced + item.counts.actions.replace + item.counts.actions.keep_both,
      identical: sum.identical + item.counts.actions.identical,
    }),
    { added: 0, replaced: 0, identical: 0 },
  );
}

/** The display name of recipe `id` in `review` (the engine's, else the id). */
export const appName = (review: PlanReview, id: string): string => review.plan.app_names[id] ?? id;

/** An offered item's label (the source's), else its key. */
export const itemLabel = (review: PlanReview, key: string): string =>
  review.offered.find((o) => o.key === key)?.label ?? key;

/**
 * Questions the engine raised once (cross-system items, plugins), kept so
 * their rows stay answered after the answer makes the engine stop asking.
 */
export interface Asked {
  readonly crossOs: Readonly<Record<ItemKey, { readonly from: OsKind; readonly to: OsKind }>>;
  readonly plugins: Readonly<Record<string, PlannedPlugin>>;
}

export const nothingAsked: Asked = { crossOs: {}, plugins: {} };

/** `asked` plus what `review` asks now (a newer plugin description wins). */
export function rememberAsked(asked: Asked, review: PlanReview): Asked {
  const crossOs = { ...asked.crossOs };
  for (const b of review.plan.blockers) {
    if (b.kind === 'cross_os') crossOs[b.item] = { from: b.from, to: b.to };
  }
  const plugins = { ...asked.plugins };
  for (const p of review.plan.plugins) plugins[p.plugin] = p;
  return { crossOs, plugins };
}

/** What the review screens are built from besides the plan. */
export interface ReviewInputs {
  readonly answers: Answers;
  readonly asked: Asked;
}

/**
 * Questions the receiver answers before approving: Accept all (Manual picks)
 * first, then secrets, open apps, other systems, plugins.
 */
export function decisions(review: PlanReview, inputs: ReviewInputs): DecisionModel[] {
  const { answers, asked } = inputs;
  const { options } = review.plan;
  const picked = (key: string): boolean => review.offered.some((o) => o.key === key && o.picked);
  const secrets: DecisionModel[] = review.offered.flatMap((o) => {
    if (o.sensitive === null) return [];
    const given = answers[o.key];
    const answer =
      given === 'bring' || given === 'leave' ? given : o.sensitive.confirmed ? 'bring' : null;
    return [
      {
        kind: 'secret' as const,
        key: o.key,
        title: o.label,
        why: review.plan.sensitive[o.key] ?? o.sensitive.reason,
        answer,
      },
    ];
  });
  const accepted = options.accept_safe_picks;
  const count = review.offered.filter((o) => o.safe_pick && o.sensitive?.confirmed !== true).length;
  // Shown while a safe pick is unconfirmed, and (answered) once Accept all was used.
  const safePicks: DecisionModel[] =
    count > 0 || accepted
      ? [{ kind: 'safe-picks', key: 'safe-picks', count, answer: accepted ? 'bring' : null }]
      : [];
  const fromApps = Object.keys(answers).filter((k) => !review.offered.some((o) => o.key === k));
  const apps = [...new Set([...review.plan.running_apps, ...fromApps])];
  const running: DecisionModel[] = apps.map((app) => ({
    kind: 'app-running' as const,
    key: app,
    app: appName(review, app),
    answer: answers[app] === 'skip' || answers[app] === 'closed' ? answers[app] : null,
  }));
  const crossOs: DecisionModel[] = Object.entries(asked.crossOs).map(([key, { from, to }]) => ({
    kind: 'cross-os' as const,
    key,
    title: itemLabel(review, key),
    from,
    to,
    answer: options.allow_cross_os.includes(key) ? 'bring' : picked(key) ? null : 'leave',
  }));
  const plugins: DecisionModel[] = Object.values(asked.plugins).map((p) => ({
    kind: 'plugin' as const,
    key: p.plugin,
    description: p.description,
    items: p.items.map((k) => itemLabel(review, k)),
    answer: options.confirmed_plugins.includes(p.plugin)
      ? 'allow'
      : p.items.some(picked)
        ? null
        : 'leave',
  }));
  return [...safePicks, ...secrets, ...running, ...crossOs, ...plugins];
}

/** The filesystem to show: the first one short of space, else the first written to. */
function spaceOf(review: PlanReview): ReviewModel['space'] {
  const list = review.space.filesystems;
  const fs = list.find((f) => f.short_by !== null) ?? list[0];
  return {
    needed: fs?.needed ?? review.space.writes,
    free: fs?.available ?? null,
    total: fs?.total ?? null,
    short: fs?.short_by !== null && fs?.short_by !== undefined,
    ignored: review.plan.options.ignore_free_space,
  };
}

/** Distinct apps of item keys (raw paths count as themselves). */
const appCount = (keys: readonly string[]): number => new Set(keys.map((k) => appOf(k) ?? k)).size;

/** Cross-system mappings in a few words: from, to, and how many apps they touch. */
function mappedOf(review: PlanReview, here: Computer): ReviewModel['mapped'] {
  const from = review.source?.os;
  const to = here.os;
  if (review.plan.mappings.length === 0 || from === undefined || to === null) return null;
  return { from, to, apps: appCount(review.plan.mappings.map((m) => m.item)) };
}

/** The rail: who moves what to whom, and which keys come along. */
function railOf(
  review: PlanReview,
  asked: readonly DecisionModel[],
  route: MoveRoute,
): ReviewModel['rail'] {
  const secrets = asked.flatMap((d) => (d.kind === 'secret' ? [d] : []));
  return {
    route,
    apps: {
      moving: appCount(review.view.items.map((i) => i.key)),
      offered: appCount(review.offered.map((o) => o.key)),
    },
    bytes: review.plan.bytes_to_write,
    keys: secrets.some((d) => d.answer === null)
      ? null
      : secrets.filter((d) => d.answer === 'bring').map((d) => d.title),
  };
}

/** "Here's what will change". `route`: the source and this computer. */
export function reviewModel(
  review: PlanReview,
  inputs: ReviewInputs,
  remote: Remote<PlanReview>,
  route: MoveRoute,
): ReviewModel {
  const asked = decisions(review, inputs);
  return {
    counts: {
      ...planCounts(review),
      left: review.offered.filter((o) => !o.picked).length,
    },
    decisions: asked,
    space: spaceOf(review),
    mapped: mappedOf(review, route.to),
    rule: review.plan.options.default_rule,
    retention: review.plan.options.retention,
    blocked: review.plan.blockers.length > 0,
    rail: railOf(review, asked, route),
    busy: remote.status !== 'ready',
    error: remote.status === 'failed' ? remote.error : null,
  };
}

/** The consent dialog for plugin `id`, from what the engine planned for it. */
export function pluginConsentModel(
  review: PlanReview,
  asked: Asked,
  id: string,
): PluginConsentModel | null {
  const p = asked.plugins[id];
  if (p === undefined) return null;
  return {
    plugin: p.plugin,
    version: p.version,
    description: p.description,
    permissions: p.permissions,
    items: p.items.map((k) => itemLabel(review, k)),
  };
}

/** Every change, per item: clashes with their rule, creates counted, remap offered. */
export function detailsModel(review: PlanReview): ReviewDetailsModel {
  const { options } = review.plan;
  const label = (key: string): string => itemLabel(review, key);
  const groups: ChangeGroup[] = review.view.items.map((item) => {
    const actions = review.plan.actions.filter((a) => a.item === item.key);
    return {
      ...appRef(appOf(item.key) ?? item.key, label(item.key)),
      id: item.key,
      rule: options.item_rules[item.key] ?? options.default_rule,
      clashes: actions.filter(isClash).map((a) => ({
        path: targetKey(a),
        display: a.abs,
        rule: options.path_rules[targetKey(a)] ?? null,
      })),
      added: item.counts.actions.create,
      canRemap: true,
      remapped: item.key in options.remaps,
    };
  });
  return {
    groups,
    choices: {
      keepBothSuffix: options.keep_both_suffix,
      retention: options.retention,
      importTrust: options.import_trust,
      keepGroupWrite: options.keep_group_write,
    },
  };
}

/** Arrived: what moved, what didn't (with the engine's reason), and what's left to install. */
export function arrivedModel(
  checklist: Checklist,
  review: PlanReview | null,
  route: MoveRoute,
): ArrivedModel {
  const picked = review?.offered.filter((o) => o.picked).map((o) => o.key) ?? checklist.moved;
  const apps = [...new Set(checklist.moved.map((k) => appOf(k) ?? k))];
  const counts = review === null ? null : planCounts(review);
  return {
    route,
    moved: apps.map((id) => (review === null ? id : appName(review, id))),
    appsTotal: Math.max(appCount(picked), apps.length),
    files: counts === null ? 0 : counts.added + counts.replaced,
    bytes: review?.plan.bytes_to_write ?? 0,
    replaced: counts?.replaced ?? 0,
    failures: checklist.skipped.map((s) => ({ what: s.what, reason: s.reason })),
    toInstall: checklist.apps_to_install,
  };
}
