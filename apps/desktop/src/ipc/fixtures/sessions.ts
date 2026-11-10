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
    created_at: 1794311910,
    error: null,
    id: '20261110-115830-eb5c9f',
    peer_os: 'linux',
    role: 'import',
    source: {
      fingerprint: {
        header: 'f6d19d2a22d4d23e9321838ab0153499c2d2aaace2f2fb1733d118ced3edae6e',
        manifest: 'c80e2c9c200449b1dd01dbd10ceb08b5b9814134a094febbcd0f8bb065add4fe',
      },
      kind: 'archive',
      path: '/home/ana/Downloads/laptop.carrova',
    },
    state: 'done',
    updated_at: 1794311911,
  },
];
