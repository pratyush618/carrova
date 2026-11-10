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
    created_at: 1794277040,
    error: null,
    id: '20261110-021720-a5b671',
    role: 'import',
    source: {
      fingerprint: {
        header: '4c23392bed43f7974d9099d0b65c5dfce2117bb17ba9f74a830713d8cce774ee',
        manifest: '71f41b1dfccdf046aa3d7cd4f8b8a33da5e2487d0b56f6ee957999924ad92cfc',
      },
      kind: 'archive',
      path: '/home/ana/Downloads/laptop.carrova',
    },
    state: 'done',
    updated_at: 1794277041,
  },
];
