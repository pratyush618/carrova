import { describe, expect, it } from 'vitest';
import { ENGINE_DEFAULTS } from '@/bindings/constants';
import type { PlanReview } from '@/bindings/PlanReview';
import { editOptions, planUnder, type PlanConditions } from '@/ipc/fake/planner';
import { planWithPicks } from '@/ipc/fake/fixtures';
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

describe('the fake planner, editing options like PlanOptions::apply_all', () => {
  const base = planUnder(plan, ENGINE_DEFAULTS.plan_options, none);

  it('answers a sensitive item: confirm keeps the default picks, leave freezes them', () => {
    const brought = editOptions(base, [
      { kind: 'answer_sensitive', item: 'ssh/keys', bring: true },
    ]);
    expect(brought.items).toBeNull();
    expect(brought.confirmed_sensitive).toEqual(['ssh/keys']);
    const left = editOptions(planUnder(plan, brought, none), [
      { kind: 'answer_sensitive', item: 'ssh/keys', bring: false },
    ]);
    expect(left.items).not.toContain('ssh/keys');
    expect(left.confirmed_sensitive).toEqual([]);
  });

  it('skips an app and brings back only its non-sensitive or confirmed items', () => {
    const skipped = editOptions(base, [
      { kind: 'answer_sensitive', item: 'ssh/config', bring: true },
      { kind: 'skip_app', app: 'ssh', skip: true },
    ]);
    expect(skipped.items?.some((k) => k.startsWith('ssh/'))).toBe(false);
    const back = editOptions(planUnder(plan, skipped, none), [
      { kind: 'skip_app', app: 'ssh', skip: false },
    ]);
    expect(back.items).toContain('ssh/config');
    expect(back.items).not.toContain('ssh/keys');
  });

  it('toggles as sets and clears per-key rules', () => {
    const edited = editOptions(base, [
      { kind: 'allow_cross_os', item: 'git/config', allow: true },
      { kind: 'allow_cross_os', item: 'git/config', allow: true },
      { kind: 'path_rule', path: 'home:.gitconfig', rule: 'skip' },
      { kind: 'path_rule', path: 'home:.gitconfig', rule: null },
      { kind: 'keep_both_suffix', suffix: '  ' },
      { kind: 'override', check: 'space' },
    ]);
    expect(edited.allow_cross_os).toEqual(['git/config']);
    expect(edited.path_rules).toEqual({});
    expect(edited.keep_both_suffix).toBe(base.plan.options.keep_both_suffix);
    expect(edited.ignore_free_space).toBe(true);
  });

  it('rejects an unknown item or plugin', () => {
    expect(() =>
      editOptions(base, [{ kind: 'item_rule', item: 'nope/x', rule: 'skip' }]),
    ).toThrow();
    expect(() =>
      editOptions(base, [{ kind: 'allow_plugin', plugin: 'nope', allow: true }]),
    ).toThrow();
  });
  it('accept all picks and confirms every safe pick, even out of a frozen list, and nothing else', () => {
    const picks = planUnder(planWithPicks, ENGINE_DEFAULTS.plan_options, none);
    const safe = picks.offered.filter((o) => o.safe_pick).map((o) => o.key);
    expect(safe).toHaveLength(2);
    const options = editOptions(picks, [
      { kind: 'answer_sensitive', item: 'ssh/keys', bring: false },
      { kind: 'accept_safe_picks' },
    ]);
    expect(options.accept_safe_picks).toBe(true);
    expect(options.confirmed_sensitive).toEqual([]);
    const accepted = planUnder(planWithPicks, options, none).offered.filter((o) => o.picked);
    expect(accepted.map((o) => o.key)).toEqual(expect.arrayContaining(safe));
    expect(accepted.every((o) => o.sensitive === null || o.sensitive.confirmed)).toBe(true);
    expect(accepted.map((o) => o.key)).not.toContain('raw:home/.ssh/id_ed25519');
  });

  it('accept all leaves a declined safe pick and brings the untouched one', () => {
    const picks = planUnder(planWithPicks, ENGINE_DEFAULTS.plan_options, none);
    const [left = '', untouched = ''] = picks.offered.filter((o) => o.safe_pick).map((o) => o.key);
    const options = editOptions(picks, [
      { kind: 'answer_sensitive', item: left, bring: false },
      { kind: 'accept_safe_picks' },
    ]);
    expect(options.declined).toEqual([left]);
    const picked = planUnder(planWithPicks, options, none).offered.filter((o) => o.picked);
    expect(picked.map((o) => o.key)).not.toContain(left);
    expect(picked.map((o) => o.key)).toContain(untouched);
    const brought = editOptions(picks, [
      { kind: 'answer_sensitive', item: left, bring: false },
      { kind: 'answer_sensitive', item: left, bring: true },
    ]);
    expect(brought.declined).toEqual([]);
  });

  it('remapping to the same place again keeps the confirmation', () => {
    const remap = { root: null, rel_prefix: 'old' };
    const moved = editOptions(base, [
      { kind: 'remap', item: 'ssh/keys', remap },
      { kind: 'answer_sensitive', item: 'ssh/keys', bring: true },
    ]);
    const again = editOptions({ ...base, plan: { ...base.plan, options: moved } }, [
      { kind: 'remap', item: 'ssh/keys', remap: { ...remap } },
    ]);
    expect(again.confirmed_sensitive).toEqual(['ssh/keys']);
  });

  it('remapping a confirmed item drops its confirmation', () => {
    const brought = editOptions(base, [
      { kind: 'answer_sensitive', item: 'ssh/keys', bring: true },
      { kind: 'remap', item: 'ssh/keys', remap: { root: null, rel_prefix: 'old' } },
    ]);
    expect(brought.confirmed_sensitive).toEqual([]);
  });
});
