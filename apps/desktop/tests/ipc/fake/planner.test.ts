import { describe, expect, it } from 'vitest';
import { ENGINE_DEFAULTS } from '@/bindings/constants';
import type { PlanReview } from '@/bindings/PlanReview';
import { planUnder, type PlanConditions } from '@/ipc/fake/planner';
import { plan } from '@/ipc/fixtures/plan';
import { planVariants } from '@/ipc/fixtures/plan-variants';

const none: PlanConditions = { runningApp: null };

/**
 * What the review screens read from a plan, minus dates in keep-both names and
 * the sensitivity of remapped items (the engine re-judges it at the new
 * target; the fake does not).
 */
function shape(review: PlanReview) {
  const remapped = (key: string) => key in review.plan.options.remaps;
  return {
    offered: review.offered.map((o) => [
      o.key,
      o.picked,
      remapped(o.key) ? 'remapped' : (o.sensitive?.confirmed ?? null),
    ]),
    actions: review.plan.actions.map((a) => [
      a.item,
      a.root,
      a.action.action === 'keep-both' ? a.rel : `${a.root}:${a.rel}`,
      a.action.action,
    ]),
    blockers: review.plan.blockers.map((b) =>
      b.kind === 'unconfirmed_sensitive' ? b.item : b.kind,
    ),
    items: review.view.items.map((i) => [i.key, i.counts.actions]),
  };
}

describe('the fake planner, held to the engine', () => {
  it.each(planVariants.map((review, i) => [i, review] as const))(
    'plans recorded variant %i as the engine did',
    (_, recorded) => {
      expect(shape(planUnder(plan, recorded.plan.options, none))).toEqual(shape(recorded));
    },
  );

  it('lifts a running-app blocker when the user overrides the check', () => {
    const running = { kind: 'running_app', app: 'neovim', name: 'Neovim' } as const;
    const replan = (ignore: boolean) =>
      planUnder(
        plan,
        { ...ENGINE_DEFAULTS.plan_options, ignore_requires_closed: ignore },
        {
          runningApp: running,
        },
      ).plan.blockers;
    expect(replan(false)).toContainEqual(running);
    expect(replan(true)).not.toContainEqual(running);
  });
});
