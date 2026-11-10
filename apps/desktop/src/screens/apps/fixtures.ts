import { NOW } from '../shared/fixtures';
import type { CatalogEntry, CatalogModel, RecipeModel } from './model';

const ENTRIES: readonly CatalogEntry[] = [
  { id: 'ssh', glyph: '>_', name: 'SSH', tier: 'verified', installed: true },
  { id: 'git', glyph: 'git', name: 'Git', tier: 'verified', installed: true },
  { id: 'vscode', glyph: '{ }', name: 'VS Code', tier: 'verified', installed: true },
  { id: 'gpg', glyph: 'gpg', name: 'GnuPG', tier: 'verified', installed: true },
  { id: 'shell', glyph: '$_', name: 'Bash and Zsh', tier: 'verified', installed: true },
  { id: 'tmux', glyph: 'tmx', name: 'tmux', tier: 'verified', installed: false },
  { id: 'nvim', glyph: 'nv', name: 'Neovim', tier: 'community', installed: false },
  { id: 'kitty', glyph: 'kt', name: 'Kitty', tier: 'community', installed: false },
];

const catalog = (filter: CatalogModel['filter']): CatalogModel => ({
  now: NOW,
  filter,
  query: '',
  entries: ENTRIES,
  bundle: { updated: new Date(2026, 10, 6, 9, 0) },
});

export const catalogAll = catalog('all');
export const catalogCommunity = catalog('community');

export const recipeSsh: RecipeModel = {
  id: 'ssh',
  glyph: '>_',
  name: 'SSH',
  description: 'Server list, known hosts and keys.',
  tier: 'verified',
  installed: true,
  items: [
    { id: 'config', label: 'Server list and shortcuts', tag: 'suggested' },
    { id: 'known-hosts', label: 'Known servers', tag: 'suggested' },
    { id: 'keys', label: 'SSH keys', tag: 'sensitive' },
    { id: 'agent', label: 'Agent socket', tag: 'unmovable' },
  ],
};
