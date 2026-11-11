// Recorded by `just ipc-fixtures`; do not edit.
import type { Picked } from '../../bindings/Picked';

export const picked: Picked = {
  outcomes: [
    {
      path: '/home/ana/.config/nvim',
      result: {
        item: 'raw:user_config/nvim',
        kind: 'added',
        replaced: [],
      },
    },
    {
      path: '/home/ana/.config/nvim/init.lua',
      result: {
        into: 'raw:user_config/nvim',
        kind: 'absorbed',
      },
    },
    {
      path: '/home/ana/.ssh/id_ed25519',
      result: {
        item: 'raw:home/.ssh/id_ed25519',
        kind: 'added',
        replaced: [],
      },
    },
    {
      path: '/home/ana/.tmux.conf',
      result: {
        item: 'raw:home/.tmux.conf',
        kind: 'added',
        replaced: [],
      },
    },
    {
      path: '/home/ana/Downloads/laptop.carrova',
      result: {
        kind: 'rejected',
        reason: {
          kind: 'outside_roots',
        },
      },
    },
  ],
  scan: {
    report: {
      apps: [],
      platform: {
        free_space: 120000000000,
        os: 'linux',
        roots: {
          cache: '/home/ana/.cache',
          documents: '/home/ana/Documents',
          home: '/home/ana',
          user_config: '/home/ana/.config',
          user_data: '/home/ana/.local/share',
        },
        snapshot: 'none',
      },
      totals: {
        default_selected_files: 0,
        default_selected_size: 0,
        recognised_size: 0,
        total_size: 0,
        unrecognised_size: 0,
      },
      unrecognised: [],
      warnings: [],
    },
    selection: {
      confirmed_plugins: [],
      export_plugins: [],
      items: {
        'raw:home/.ssh/id_ed25519': {
          confirmed_sensitive: false,
          excluded_paths: [],
          force_unmovable: false,
          included: false,
          paths: ['home:.ssh/id_ed25519'],
          pick: {
            files: 1,
            kind: 'file',
            size: 41,
          },
          sensitive: true,
          suggested: false,
          unmovable: null,
        },
        'raw:home/.tmux.conf': {
          confirmed_sensitive: false,
          excluded_paths: [],
          force_unmovable: false,
          included: true,
          paths: ['home:.tmux.conf'],
          pick: {
            files: 1,
            kind: 'file',
            size: 16,
          },
          sensitive: false,
          suggested: false,
          unmovable: null,
        },
        'raw:user_config/nvim': {
          confirmed_sensitive: false,
          excluded_paths: [],
          force_unmovable: false,
          included: true,
          paths: ['user_config:nvim'],
          pick: {
            files: 2,
            kind: 'dir',
            size: 30,
          },
          sensitive: false,
          suggested: false,
          unmovable: null,
        },
      },
    },
    session: '20261111-075959-ee36f9',
    view: {
      hidden_raw: 0,
      included: 2,
      included_size: 46,
      issues: [],
      picks: [
        {
          display: '~/.ssh/id_ed25519',
          facts: {
            files: 1,
            kind: 'file',
            size: 41,
          },
          included: false,
          key: 'raw:home/.ssh/id_ed25519',
          tags: [
            {
              confirmed: false,
              kind: 'sensitive',
            },
          ],
        },
        {
          display: '~/.tmux.conf',
          facts: {
            files: 1,
            kind: 'file',
            size: 16,
          },
          included: true,
          key: 'raw:home/.tmux.conf',
          tags: [],
        },
        {
          display: '~/.config/nvim',
          facts: {
            files: 2,
            kind: 'dir',
            size: 30,
          },
          included: true,
          key: 'raw:user_config/nvim',
          tags: [],
        },
      ],
      plugins: [],
      rows: [],
    },
  },
};
