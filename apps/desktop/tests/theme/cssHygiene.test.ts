import { readFileSync, readdirSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';

const SRC = join(import.meta.dirname, '../../src');

function cssFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return cssFiles(path);
    return entry.name.endsWith('.css') ? [path] : [];
  });
}

// Raw colours belong in tokens.ts only. Physical box properties break RTL.
const RAW_COLOUR = /#[0-9a-f]{3,8}\b|\brgba?\(|\bhsla?\(/i;
const PHYSICAL =
  /(?:^|[\s;{])(?:margin|padding|border)-(?:left|right|top|bottom)\b|(?:^|[\s;{])(?:left|right|top|bottom)\s*:|text-align:\s*(?:left|right)/m;

const files = cssFiles(SRC);

describe('component stylesheets', () => {
  it('exist', () => {
    expect(files.length).toBeGreaterThan(20);
  });

  for (const file of files) {
    const name = relative(SRC, file);
    const css = readFileSync(file, 'utf8');

    it(`${name} uses colour tokens, never raw colours`, () => {
      expect(css).not.toMatch(RAW_COLOUR);
    });

    it(`${name} uses logical properties only`, () => {
      expect(css).not.toMatch(PHYSICAL);
    });
  }
});
