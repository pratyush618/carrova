import type { OverrideModel } from './model';

export const OVERRIDES = {
  unmovable: {
    kind: 'unmovable',
    item: 'VS Code extensions',
    reason:
      'Extensions are programs built for one system and one VS Code version. Copied ones often fail to start.',
    count: 12,
  },
  running: { kind: 'running', app: 'VS Code' },
  space: { kind: 'space' },
  elsewhere: { kind: 'elsewhere' },
} as const satisfies Record<OverrideModel['kind'], OverrideModel>;
