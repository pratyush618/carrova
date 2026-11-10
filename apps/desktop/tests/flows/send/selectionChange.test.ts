import { describe, expect, it } from 'vitest';
import { toEdits } from '@/flows/send/selectionChange';
import { scan } from '@/ipc/fixtures/scan';

const { selection } = scan;

describe('screen changes as engine edits', () => {
  it('ticking a sensitive item in Customize also confirms it', () => {
    expect(toEdits({ kind: 'item', key: 'ssh/keys', included: true }, selection)).toEqual([
      { kind: 'item', item: 'ssh/keys', included: true },
      { kind: 'confirm_sensitive', item: 'ssh/keys', confirmed: true },
    ]);
    expect(toEdits({ kind: 'item', key: 'git/config', included: false }, selection)).toEqual([
      { kind: 'item', item: 'git/config', included: false },
    ]);
  });

  it('answering a sensitive ask confirms (or withdraws) and ticks together', () => {
    expect(toEdits({ kind: 'sensitive', key: 'ssh/keys', bring: false }, selection)).toEqual([
      { kind: 'confirm_sensitive', item: 'ssh/keys', confirmed: false },
      { kind: 'item', item: 'ssh/keys', included: false },
    ]);
  });

  it('forcing an unmovable item overrides it and ticks it', () => {
    expect(toEdits({ kind: 'force', key: 'tmux/plugins' }, selection)).toEqual([
      { kind: 'force_unmovable', item: 'tmux/plugins' },
      { kind: 'item', item: 'tmux/plugins', included: true },
    ]);
  });

  it('passes app rows, paths and caches through', () => {
    expect(toEdits({ kind: 'app', app: 'git', included: false }, selection)).toEqual([
      { kind: 'app', app: 'git', included: false },
    ]);
    expect(
      toEdits(
        { kind: 'path', key: 'git/config', path: 'home:.gitconfig', included: false },
        selection,
      ),
    ).toEqual([{ kind: 'path', item: 'git/config', path: 'home:.gitconfig', included: false }]);
  });
});
