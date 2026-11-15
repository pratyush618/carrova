import { readFileSync } from 'node:fs';
import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { CodeDisplay } from '@/ui';

const css = readFileSync('src/ui/CodeDisplay/CodeDisplay.css', 'utf8');
const rule = (selector: string) =>
  new RegExp(`^${selector.replace('.', '\\.')} \\{([^}]*)\\}`, 'm').exec(css)?.[1] ?? '';

describe('CodeDisplay long codes', () => {
  it('keeps each word as one group so a wrap never splits a word', () => {
    render(<CodeDisplay words={['28', 'helmet', 'wholesaler']} />);
    const groups = screen
      .getByRole('img', { name: /28 helmet wholesaler/ })
      .querySelectorAll('.p-code__word');
    expect(Array.from(groups, (g) => g.querySelectorAll('.p-flap').length)).toEqual([2, 6, 10]);
    expect(Array.from(groups, (g) => g.querySelectorAll('.p-code__dash').length)).toEqual([
      1, 1, 0,
    ]);
  });

  it('lets the row wrap while words and tiles never shrink', () => {
    expect(rule('.p-code')).toContain('flex-wrap: wrap');
    expect(rule('.p-code__word')).toContain('flex: none');
    expect(rule('.p-flap')).toContain('flex: none');
  });
});
