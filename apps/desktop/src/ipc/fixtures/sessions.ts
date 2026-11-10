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
    created_at: 1794286820,
    error: null,
    id: '20261110-050020-811a4c',
    role: 'import',
    source: {
      fingerprint: {
        header: '0995ac3184a51bf5113fee9f0a8084955ed9fe638a06dc1c42befce799179969',
        manifest: '5684dde393b0abb7219d33d8862fca3991ffbcc2b4c43241f373813e1a56a097',
      },
      kind: 'archive',
      path: '/home/ana/Downloads/laptop.carrova',
    },
    state: 'done',
    updated_at: 1794286821,
  },
];
