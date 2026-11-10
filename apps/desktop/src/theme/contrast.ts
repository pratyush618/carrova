import type { Hex } from './tokens';

/** sRGB channel (0–255) to linear light, per WCAG 2.x. */
function linear(channel: number): number {
  const c = channel / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

/** Parses `#RGB` or `#RRGGBB`. Throws on anything else: the token table is static. */
export function parseHex(hex: Hex): readonly [number, number, number] {
  const body = hex.slice(1);
  const full =
    body.length === 3
      ? body
          .split('')
          .map((c) => c + c)
          .join('')
      : body;
  if (!/^[0-9a-f]{6}$/i.test(full)) {
    throw new Error(`not a hex colour: ${hex}`);
  }
  const n = Number.parseInt(full, 16);
  return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

/** WCAG relative luminance of a colour. */
export function relativeLuminance(hex: Hex): number {
  const [r, g, b] = parseHex(hex);
  return 0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b);
}

/** WCAG contrast ratio between two colours (1–21). */
export function contrastRatio(a: Hex, b: Hex): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}
