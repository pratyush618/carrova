import { NOW } from '../shared/fixtures';
import type { CatalogDetail, CatalogEntry, CatalogGroup, CatalogModel, TierFilter } from './model';

const app = (
  id: string,
  glyph: string,
  name: string,
  tier: CatalogEntry['tier'],
  here: boolean,
): CatalogEntry => ({ id, glyph, name, tier, here });

const GIT = app('git', 'git', 'Git', 'verified', true);
const VSCODE = app('vscode', '{ }', 'VS Code', 'verified', true);
const VSCODIUM = app('vscodium', 'vsc', 'VSCodium', 'community', false);

const GROUPS: readonly CatalogGroup[] = [
  {
    category: 'dev-tools',
    total: 41,
    shown: [
      GIT,
      VSCODE,
      app('nvim', 'nv', 'Neovim', 'community', true),
      app('docker', 'dk', 'Docker', 'verified', true),
      app('idea', 'jb', 'IntelliJ IDEA', 'community', false),
    ],
  },
  {
    category: 'shells',
    total: 12,
    shown: [
      app('zsh', '$_', 'Zsh and Oh My Zsh', 'verified', true),
      app('tmux', 'tmx', 'tmux', 'verified', false),
    ],
  },
];

const catalog = (
  filter: TierFilter,
  groups: readonly CatalogGroup[],
  extra: Partial<CatalogModel> = {},
): CatalogModel => ({
  now: NOW,
  filter,
  query: '',
  groups,
  selected: null,
  bundle: { updated: new Date(2026, 10, 6, 9, 0) },
  ...extra,
});

const vscodeDetail: CatalogDetail = {
  ...VSCODE,
  description: 'Settings, keybindings and snippets.',
  moves: [
    { id: 'user', label: 'Settings, keybindings and snippets', tag: 'suggested' },
    { id: 'extensions', label: 'Extensions list', tag: 'suggested' },
  ],
  leftOut: [
    { id: 'workspace', label: 'Workspace storage and history', tag: null },
    { id: 'extension-files', label: 'Extension files', tag: 'unmovable' },
  ],
};

export const catalogAll = catalog('all', GROUPS);
export const catalogCommunity = catalog(
  'community',
  GROUPS.map((g) => ({ ...g, shown: g.shown.filter((e) => e.tier === 'community') })),
);
export const catalogSelected = catalog('all', GROUPS, { selected: vscodeDetail });
export const catalogSearch = catalog(
  'all',
  [{ category: 'dev-tools', total: 2, shown: [VSCODE, VSCODIUM] }],
  { query: 'vs' },
);

/** A sensitive app: its keys start off and say so. */
export const catalogSensitive = catalog('all', GROUPS, {
  selected: {
    ...app('ssh', '>_', 'SSH', 'verified', true),
    description: 'Server list, known hosts and keys.',
    moves: [
      { id: 'config', label: 'Server list and shortcuts', tag: 'suggested' },
      { id: 'keys', label: 'SSH keys', tag: 'sensitive' },
    ],
    leftOut: [{ id: 'agent', label: 'Agent socket', tag: 'unmovable' }],
  },
});
