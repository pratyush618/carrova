// Recorded by `just ipc-fixtures`; do not edit.
import type { RollbackReport } from '../../bindings/RollbackReport';

export const rollback: RollbackReport = {
  failed: [],
  quarantined: [],
  removed: [
    '/home/ana/.config/nvim/lua/plugins.lua',
    '/home/ana/.ssh/id_ed25519.pub',
    '/home/ana/.ssh/id_ed25519',
    '/home/ana/.ssh/config',
    '/home/ana/.config/nvim/lua',
    '/home/ana/.config/nvim/init.lua',
    '/home/ana/.tmux.conf',
    '/home/ana/.config/starship.toml',
    '/home/ana/.ssh',
    '/home/ana/.config/nvim',
    '/home/ana/.gitconfig',
    '/home/ana/.bashrc',
  ],
  restored: ['/home/ana/.gitconfig'],
};
