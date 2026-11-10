// Recorded by `just ipc-fixtures`; do not edit.
import type { SelectionEdit } from '../../bindings/SelectionEdit';

export const selectEditsInput: SelectionEdit[] = [
  {
    app: 'git',
    included: false,
    kind: 'app',
  },
  {
    included: false,
    item: 'neovim/config',
    kind: 'path',
    path: 'user_config:nvim/lua',
  },
  {
    included: false,
    item: 'neovim/config',
    kind: 'path',
    path: 'user_config:nvim/after',
  },
  {
    included: true,
    kind: 'caches',
  },
  {
    confirmed: true,
    item: 'ssh/keys',
    kind: 'confirm_sensitive',
  },
  {
    included: true,
    item: 'ssh/keys',
    kind: 'item',
  },
];
