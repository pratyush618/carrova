import { describe, expect, it } from 'vitest';
import { contrastRatio, parseHex, relativeLuminance } from '@/theme/contrast';
import { colorTokens, contrastPairs } from '@/theme/tokens';
import type { ThemeName } from '@/theme/css';

const THEMES: readonly ThemeName[] = ['light', 'dark'];

describe('contrast maths', () => {
  it('matches the WCAG extremes for black and white', () => {
    expect(relativeLuminance('#000000')).toBe(0);
    expect(relativeLuminance('#FFFFFF')).toBeCloseTo(1, 10);
    expect(contrastRatio('#000', '#fff')).toBeCloseTo(21, 10);
  });

  it('is symmetric in its arguments', () => {
    expect(contrastRatio('#3346D3', '#F7F8FA')).toBeCloseTo(contrastRatio('#F7F8FA', '#3346D3'));
  });

  it('rejects malformed colours', () => {
    expect(() => parseHex('#12')).toThrow(/not a hex colour/);
  });

  it('reproduces a ratio published in the mockup README', () => {
    // text-primary on surface-1, light: 16.74:1.
    expect(contrastRatio('#15181D', '#F7F8FA')).toBeCloseTo(16.74, 1);
  });
});

describe('token table', () => {
  for (const theme of THEMES) {
    for (const { fg, bg, min } of contrastPairs) {
      it(`${theme}: ${fg} on ${bg} meets ${String(min)}:1`, () => {
        const ratio = contrastRatio(colorTokens[fg][theme], colorTokens[bg][theme]);
        expect(ratio).toBeGreaterThanOrEqual(min);
      });
    }
  }
});
