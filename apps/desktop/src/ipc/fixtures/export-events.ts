// Recorded by `just ipc-fixtures`; do not edit.
import type { BridgeEvent } from '../../bindings/BridgeEvent';

export const exportEvents: BridgeEvent[] = [
  {
    event: {
      event: 'state_changed',
      from: 'scanned',
      session: '20261110-021713-205b40',
      to: 'selected',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      event: 'state_changed',
      from: 'selected',
      session: '20261110-021713-205b40',
      to: 'transferring',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      done: 0,
      event: 'progress',
      phase: 'capture',
      total: 250,
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      bytes: 37,
      event: 'item_captured',
      files: 1,
      item: 'bash/config',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      bytes: 44,
      event: 'item_captured',
      files: 1,
      item: 'git/config',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      bytes: 30,
      event: 'item_captured',
      files: 2,
      item: 'neovim/config',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      bytes: 30,
      event: 'item_captured',
      files: 1,
      item: 'ssh/config',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      bytes: 73,
      event: 'item_captured',
      files: 2,
      item: 'ssh/keys',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      bytes: 20,
      event: 'item_captured',
      files: 1,
      item: 'starship/config',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      done: 250,
      event: 'progress',
      phase: 'capture',
      total: 250,
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      bytes: 16,
      event: 'item_captured',
      files: 1,
      item: 'tmux/config',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      done: 0,
      event: 'progress',
      phase: 'archive',
      total: 250,
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      done: 37,
      event: 'progress',
      phase: 'archive',
      total: 250,
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      done: 250,
      event: 'progress',
      phase: 'archive',
      total: 250,
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
      event: 'state_changed',
      from: 'transferring',
      session: '20261110-021713-205b40',
      to: 'done',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    event: {
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
        rollback_available: false,
        skipped: [],
      },
      event: 'finished',
    },
    kind: 'engine',
    session: '20261110-021713-205b40',
  },
  {
    error: null,
    kind: 'task_ended',
    session: '20261110-021713-205b40',
  },
];
