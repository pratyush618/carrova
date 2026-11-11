// Recorded by `just ipc-fixtures`; do not edit.
import type { SessionRecord } from '../../bindings/SessionRecord';

export const sessions: SessionRecord[] = [
  {
    approved: true,
    bytes_moved: 250,
    checklist: {
      apps_to_install: [],
      moved: [
        'bash/config',
        'git/config',
        'neovim/config',
        'ssh/config',
        'ssh/keys',
        'starship/config',
        'tmux/config',
      ],
      rollback_available: true,
      skipped: [],
    },
    created_at: 1794384001,
    error: null,
    id: '20261111-080001-d0928e',
    peer_os: 'linux',
    role: 'import',
    source: {
      fingerprint: {
        header: '775be581242c7c5cb2ee3b5dbca3372c02fdd4586289b85b7ae19a90efa918a4',
        manifest: 'b19f1db6a7917982d9a68ab91460b66e92c6812b283861b0c9fec76a5be55521',
      },
      kind: 'archive',
      path: '/home/ana/Downloads/laptop.carrova',
    },
    state: 'done',
    updated_at: 1794384004,
  },
];
