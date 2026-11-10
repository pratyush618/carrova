import { describe, expect, it } from 'vitest';
import { ENGINE_DEFAULTS } from '@/bindings/constants';
import type { PlanReview } from '@/bindings/PlanReview';
import { plan } from '@/ipc/fixtures/plan';
import { planUnconfirmed } from '@/ipc/fixtures/plan-unconfirmed';
import { allowCrossOs, allowPlugin, setPathRule } from '@/flows/receive/planOptions';
import {
  arrivedModel,
  decisions,
  detailsModel,
  nothingAsked,
  pluginConsentModel,
  rememberAsked,
  reviewModel,
  type ReviewInputs,
} from '@/flows/receive/reviewViews';
import type { MoveRoute } from '@/screens/shared/model';

const inputs = (over: Partial<ReviewInputs> = {}): ReviewInputs => ({
  options: ENGINE_DEFAULTS.plan_options,
  answers: {},
  asked: nothingAsked,
  ...over,
});
const route: MoveRoute = {
  from: { name: 'ana-desktop', os: 'mac_os' },
  to: { name: 'ana-laptop', os: 'linux' },
};
const [first, second] = plan.offered;
if (first === undefined || second === undefined) throw new Error('fixture offers too little');

/** `plan` with `blockers`, and `first` the one item `path-rewrite` changes. */
function withBlockers(blockers: PlanReview['plan']['blockers']): PlanReview {
  return {
    ...plan,
    plan: {
      ...plan.plan,
      blockers,
      plugins: [
        {
          plugin: 'path-rewrite',
          version: '1.0.0',
          description: 'Rewrites paths.',
          permissions: ['It cannot use the network.'],
          items: [first?.key ?? ''],
        },
      ],
    },
  };
}
const crossOs = withBlockers([{ kind: 'cross_os', item: second.key, from: 'mac_os', to: 'linux' }]);
const pluginBlocked = withBlockers([
  { kind: 'unconfirmed_plugin', plugin: 'path-rewrite', items: [first.key] },
]);
const unpicked = (review: PlanReview, key: string): PlanReview => ({
  ...review,
  offered: review.offered.map((o) => (o.key === key ? { ...o, picked: false } : o)),
  plan: { ...review.plan, blockers: [] },
});

describe('review views', () => {
  it('asks about each sensitive item with the receiver-side reason', () => {
    const secrets = decisions(plan, inputs()).flatMap((d) => (d.kind === 'secret' ? [d] : []));
    const expected = plan.offered.flatMap((o) =>
      o.sensitive === null ? [] : [[o.key, plan.plan.sensitive[o.key] ?? o.sensitive.reason]],
    );
    expect(secrets.map((d) => [d.key, d.why])).toEqual(expected);
  });

  it('keeps an open-app decision answered after the user skipped the app', () => {
    const asked = decisions(plan, inputs({ answers: { neovim: 'skip' } }));
    expect(asked.find((d) => d.kind === 'app-running')).toMatchObject({
      key: 'neovim',
      answer: 'skip',
    });
  });

  it('asks about an item made for the other system until it is left or brought anyway', () => {
    const asked = rememberAsked(nothingAsked, crossOs);
    const row = (review: PlanReview, options = ENGINE_DEFAULTS.plan_options) =>
      decisions(review, inputs({ options, asked })).find((d) => d.kind === 'cross-os');
    expect(row(crossOs)).toMatchObject({
      key: second.key,
      title: second.label,
      from: 'mac_os',
      to: 'linux',
      answer: null,
    });
    // Answered, the engine stops asking; the row stays with the answer.
    expect(row(unpicked(crossOs, second.key))?.answer).toBe('leave');
    const allowed = allowCrossOs(ENGINE_DEFAULTS.plan_options, second.key);
    expect(row({ ...crossOs, plan: { ...crossOs.plan, blockers: [] } }, allowed)?.answer).toBe(
      'bring',
    );
  });

  it('asks to allow a plugin by name, with the items it changes, and remembers the answer', () => {
    const asked = rememberAsked(nothingAsked, pluginBlocked);
    const row = (review: PlanReview, options = ENGINE_DEFAULTS.plan_options) =>
      decisions(review, inputs({ options, asked })).find((d) => d.kind === 'plugin');
    expect(row(pluginBlocked)).toEqual({
      kind: 'plugin',
      key: 'path-rewrite',
      description: 'Rewrites paths.',
      items: [first.label],
      answer: null,
    });
    const allowed = allowPlugin(ENGINE_DEFAULTS.plan_options, 'path-rewrite');
    expect(row(pluginBlocked, allowed)?.answer).toBe('allow');
    const noPlugin = { ...unpicked(pluginBlocked, first.key) };
    expect(row({ ...noPlugin, plan: { ...noPlugin.plan, plugins: [] } })?.answer).toBe('leave');
  });

  it('shows the consent dialog from what the engine planned for the plugin', () => {
    const asked = rememberAsked(nothingAsked, pluginBlocked);
    expect(pluginConsentModel(plan, asked, 'path-rewrite')).toEqual({
      plugin: 'path-rewrite',
      version: '1.0.0',
      description: 'Rewrites paths.',
      permissions: ['It cannot use the network.'],
      items: [first.label],
    });
    expect(pluginConsentModel(plan, asked, 'other')).toBeNull();
  });

  it('blocks Approve while re-planning or while the engine still blocks', () => {
    const running = { kind: 'running_app', app: 'neovim', name: 'Neovim' } as const;
    const blocked = { ...plan, plan: { ...plan.plan, blockers: [running] } };
    expect(reviewModel(blocked, inputs(), true, route)).toMatchObject({
      busy: true,
      blocked: true,
    });
    expect(reviewModel(plan, inputs(), false, route).blocked).toBe(false);
  });

  it('sums cross-system mappings into one suggestion, by app', () => {
    const mapped: PlanReview = {
      ...plan,
      source: { os: 'mac_os', device_name: 'ana-desktop' },
      plan: {
        ...plan.plan,
        mappings: [
          {
            item: 'neovim/config',
            from: ['user_data', 'nvim'],
            to: ['user_config', 'nvim'],
            to_abs: null,
          },
          { item: 'neovim/config', from: ['cache', 'nvim'], to: ['cache', 'nvim'], to_abs: null },
        ],
      },
    };
    expect(reviewModel(mapped, inputs(), false, route).mapped).toEqual({
      from: 'mac_os',
      to: 'linux',
      apps: 1,
    });
    expect(reviewModel(plan, inputs(), false, route).mapped).toBeNull();
  });

  it('says on the rail which keys come along once every one is answered', () => {
    const secrets = planUnconfirmed.offered.filter((o) => o.sensitive !== null);
    expect(reviewModel(planUnconfirmed, inputs(), false, route).rail.keys).toBeNull();
    const answers = Object.fromEntries(
      secrets.map((o, i) => [o.key, i === 0 ? ('bring' as const) : ('leave' as const)]),
    );
    expect(reviewModel(planUnconfirmed, inputs({ answers }), false, route).rail.keys).toEqual([
      secrets[0]?.label,
    ]);
  });

  it('shows a per-file rule on its clash and the item rule elsewhere', () => {
    const options = ENGINE_DEFAULTS.plan_options;
    const groups = detailsModel(plan, options).groups.filter((g) => g.clashes.length > 0);
    const [group] = groups;
    const clash = group?.clashes[0];
    if (group === undefined || clash === undefined) throw new Error('fixture has no clash');
    expect(group.rule).toBe('replace');
    const ruled = detailsModel(plan, setPathRule(options, clash.path, 'skip'));
    expect(ruled.groups.find((g) => g.id === group.id)?.clashes[0]?.rule).toBe('skip');
  });

  it('lists apps to install, what arrived, and skipped items as failures', () => {
    const model = arrivedModel(
      {
        moved: ['git/config', 'neovim/config'],
        skipped: [{ what: 'gpg/trust', reason: 'locked' }],
        apps_to_install: ['Neovim'],
        rollback_available: true,
      },
      plan,
      route,
    );
    expect(model.toInstall).toEqual(['Neovim']);
    expect(model.failures).toEqual([{ what: 'gpg/trust', reason: 'locked' }]);
    expect(model.moved).toEqual(['git', 'neovim']);
    expect(model.route).toBe(route);
  });
});
