import type { Checklist, PlanOptions, PlanReview } from '../../engine/types';
import type {
  ArrivedModel,
  BlockerModel,
  ChangeGroup,
  DecisionAnswer,
  DecisionModel,
  ReviewDetailsModel,
  ReviewModel,
} from '../../screens/receive/model';
import { appOf } from '../shared/keys';
import { appRef } from '../shared/paths';

/** The user's answers to review decisions: item keys (secrets) and app ids (open apps). */
export type Answers = Readonly<Record<string, DecisionAnswer>>;

type Action = PlanReview['plan']['actions'][number];

const isClash = (a: Action): boolean =>
  a.action.action === 'replace' || a.action.action === 'keep-both' || a.action.action === 'skip';

const targetKey = (a: Action): string => `${a.root}:${a.rel}`;

/** Totals over the whole plan, as the three facts on the review screen. */
export function planCounts(review: PlanReview): ReviewModel['counts'] {
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

/** Questions the receiver answers before approving: sensitive items, then open apps. */
export function decisions(review: PlanReview, answers: Answers): DecisionModel[] {
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
  const asked = Object.keys(answers).filter((k) => !review.offered.some((o) => o.key === k));
  const apps = [...new Set([...review.plan.running_apps, ...asked])];
  const running: DecisionModel[] = apps.map((app) => ({
    kind: 'app-running' as const,
    key: app,
    app: appName(review, app),
    answer: answers[app] === 'skip' || answers[app] === 'closed' ? answers[app] : null,
  }));
  return [...secrets, ...running];
}

/** The filesystem to show: the first one short of space, else the first written to. */
function spaceOf(review: PlanReview, options: PlanOptions): ReviewModel['space'] {
  const list = review.space.filesystems;
  const fs = list.find((f) => f.short_by !== null) ?? list[0];
  return {
    needed: fs?.needed ?? review.space.writes,
    free: fs?.available ?? null,
    short: fs?.short_by !== null && fs?.short_by !== undefined,
    ignored: options.ignore_free_space,
  };
}

/** The engine's blockers, with the names the user knows. */
function blockersOf(review: PlanReview): BlockerModel[] {
  const label = (key: string): string => review.offered.find((o) => o.key === key)?.label ?? key;
  return review.plan.blockers.map((b) => {
    switch (b.kind) {
      case 'unconfirmed_sensitive':
        return { kind: b.kind, what: label(b.item), why: b.reason };
      case 'running_app':
        return { kind: b.kind, app: b.name };
      case 'free_space':
        return { kind: b.kind, needed: b.needed, free: b.available };
    }
  });
}

/** "Here's what will change". */
export function reviewModel(
  review: PlanReview,
  options: PlanOptions,
  answers: Answers,
  busy: boolean,
): ReviewModel {
  return {
    counts: planCounts(review),
    undoDays: null,
    decisions: decisions(review, answers),
    space: spaceOf(review, options),
    blockers: blockersOf(review),
    busy,
  };
}

/** Every change, per item: clashes with their rule, creates counted, remap offered. */
export function detailsModel(review: PlanReview, options: PlanOptions): ReviewDetailsModel {
  const label = (key: string): string => review.offered.find((o) => o.key === key)?.label ?? key;
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

/** Arrived: what moved, what didn't (with the engine's reason), and what's left to do. */
export function arrivedModel(
  checklist: Checklist,
  review: PlanReview | null,
  peer: string,
): ArrivedModel {
  const apps = (keys: readonly string[]) => new Set(keys.map((k) => appOf(k) ?? k)).size;
  const picked = review?.offered.filter((o) => o.picked).map((o) => o.key) ?? checklist.moved;
  return {
    peer,
    apps: apps(checklist.moved),
    appsTotal: Math.max(apps(picked), apps(checklist.moved)),
    bytes: review?.plan.bytes_to_write ?? 0,
    replaced: review === null ? 0 : planCounts(review).replaced,
    failures: checklist.skipped.map((s) => ({ what: s.what, reason: s.reason })),
    tasks: checklist.apps_to_install.map((app) => ({ kind: 'install' as const, app })),
  };
}
