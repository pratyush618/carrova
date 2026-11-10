import { describe, expect, it } from 'vitest';
import { ENGINE_DEFAULTS } from '@/bindings/constants';
import { plan } from '@/ipc/fixtures/plan';
import { setPathRule } from '@/flows/receive/planOptions';
import { arrivedModel, decisions, detailsModel, reviewModel } from '@/flows/receive/reviewViews';

describe('review views', () => {
  it('asks about each sensitive item with the receiver-side reason', () => {
    const secrets = decisions(plan, {}).flatMap((d) => (d.kind === 'secret' ? [d] : []));
    const expected = plan.offered.flatMap((o) =>
      o.sensitive === null ? [] : [[o.key, plan.plan.sensitive[o.key] ?? o.sensitive.reason]],
    );
    expect(secrets.map((d) => [d.key, d.why])).toEqual(expected);
  });

  it('keeps an open-app decision answered after the user skipped the app', () => {
    const asked = decisions(plan, { neovim: 'skip' });
    expect(asked.find((d) => d.kind === 'app-running')).toMatchObject({
      key: 'neovim',
      answer: 'skip',
    });
  });

  it('marks Approve busy while re-planning and lists blockers', () => {
    const running = { kind: 'running_app', app: 'neovim', name: 'Neovim' } as const;
    const [secret] = plan.offered.filter((o) => o.sensitive !== null);
    if (secret === undefined) throw new Error('fixture has no sensitive item');
    const unconfirmed = {
      kind: 'unconfirmed_sensitive',
      item: secret.key,
      reason: 'keys',
    } as const;
    const blocked = { ...plan, plan: { ...plan.plan, blockers: [running, unconfirmed] } };
    expect(reviewModel(blocked, ENGINE_DEFAULTS.plan_options, {}, true)).toMatchObject({
      busy: true,
      blockers: [
        { kind: 'running_app', app: 'Neovim' },
        { kind: 'unconfirmed_sensitive', what: secret.label, why: 'keys' },
      ],
    });
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

  it('turns apps to install into tasks and skipped items into failures', () => {
    const model = arrivedModel(
      {
        moved: ['git/config'],
        skipped: [{ what: 'gpg/trust', reason: 'locked' }],
        apps_to_install: ['Neovim'],
        rollback_available: true,
      },
      plan,
      'old-laptop',
    );
    expect(model.tasks).toEqual([{ kind: 'install', app: 'Neovim' }]);
    expect(model.failures).toEqual([{ what: 'gpg/trust', reason: 'locked' }]);
    expect(model.apps).toBe(1);
  });
});
