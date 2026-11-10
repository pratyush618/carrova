import { describe, expect, it } from 'vitest';
import { splitChange } from '@/ui/MapRow/diff';

const joined = ({ before, changed, after }: { before: string; changed: string; after: string }) =>
  before + changed + after;

describe('splitChange', () => {
  it('highlights only the segments that differ', () => {
    const [from, to] = splitChange(
      '~/.config/Code/User',
      '~/Library/Application Support/Code/User',
    );
    expect(from).toEqual({ before: '~/', changed: '.config', after: '/Code/User' });
    expect(to).toEqual({
      before: '~/',
      changed: 'Library/Application Support',
      after: '/Code/User',
    });
  });

  it('reports nothing changed for the same place', () => {
    const [from, to] = splitChange('~/.gitconfig', '~/.gitconfig');
    expect(from.changed).toBe('');
    expect(to.changed).toBe('');
    expect(joined(to)).toBe('~/.gitconfig');
  });

  it('keeps every path intact when one side only inserts or drops segments', () => {
    const cases: readonly (readonly [string, string])[] = [
      ['~/a/b', '~/x/a/b'],
      ['~/a', '~/a/b'],
      ['a/b', 'c/d'],
      ['~/.zshrc', '~/.config/zsh/.zshrc'],
    ];
    for (const [a, b] of cases) {
      const [from, to] = splitChange(a, b);
      expect(joined(from)).toBe(a);
      expect(joined(to)).toBe(b);
    }
    const [, inserted] = splitChange('~/a/b', '~/x/a/b');
    expect(inserted.changed).toBe('x');
  });
});
