import { describe, expect, it } from 'vitest';
import type { PlanReview } from '@/bindings/PlanReview';
import { mappingModel } from '@/flows/receive/mappingViews';
import { plan } from '@/ipc/fixtures/plan';

const fromAMac: PlanReview = {
  ...plan,
  source: { os: 'mac_os', device_name: 'ana-desktop' },
  plan: {
    ...plan.plan,
    mappings: [
      {
        item: 'neovim/config',
        from: ['user_data', 'nvim'],
        to: ['user_config', 'nvim'],
        to_abs: '/home/ana/.config/nvim',
      },
      {
        item: 'git/config',
        from: ['home', '.gitconfig'],
        to: ['home', '.gitconfig'],
        to_abs: '/home/ana/.gitconfig',
      },
    ],
  },
};

describe('mapping views', () => {
  it('puts items that change place first and folds the ones that keep it', () => {
    const model = mappingModel(fromAMac, 'linux');
    expect(model?.moved.map((r) => r.title)).toEqual([
      plan.offered.find((o) => o.key === 'neovim/config')?.label,
    ]);
    expect(model?.same.map((r) => r.key)).toEqual(['git/config#1']);
  });

  it("shows the target's real folder here, and the source's as a semantic one", () => {
    const [row] = mappingModel(fromAMac, 'linux')?.moved ?? [];
    expect(row?.from).toEqual({ root: 'user_data', rel: 'nvim', abs: null });
    expect(row?.to).toEqual({ root: 'user_config', rel: 'nvim', abs: '/home/ana/.config/nvim' });
  });

  it('needs to know where the archive came from', () => {
    expect(mappingModel({ ...fromAMac, source: null }, 'linux')).toBeNull();
  });
});
