// Recorded by `just ipc-fixtures`; do not edit.
import type { ScanResult } from '../../bindings/ScanResult';

export const manual: ScanResult = {
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
    items: {},
  },
  session: '20261111-075959-ee36f9',
  view: {
    hidden_raw: 0,
    included: 0,
    included_size: 0,
    issues: [],
    picks: [],
    plugins: [],
    rows: [],
  },
};
