import { mapMessages, type Catalog } from './catalog';

/** Pseudo-locale code (Unicode's reserved "accented English"). */
export const PSEUDO_LOCALE = 'en-XA';

const ACCENTED: Readonly<Record<string, string>> = {
  a: 'á',
  b: 'ƀ',
  c: 'ç',
  d: 'ð',
  e: 'é',
  f: 'ƒ',
  g: 'ĝ',
  h: 'ĥ',
  i: 'í',
  j: 'ĵ',
  k: 'ķ',
  l: 'ļ',
  m: 'ɱ',
  n: 'ñ',
  o: 'ó',
  p: 'þ',
  q: 'ǫ',
  r: 'ŕ',
  s: 'š',
  t: 'ŧ',
  u: 'ú',
  v: 'ṽ',
  w: 'ŵ',
  x: 'ẋ',
  y: 'ý',
  z: 'ž',
  A: 'Á',
  B: 'Ɓ',
  C: 'Ç',
  D: 'Ð',
  E: 'É',
  F: 'Ƒ',
  G: 'Ĝ',
  H: 'Ĥ',
  I: 'Í',
  J: 'Ĵ',
  K: 'Ķ',
  L: 'Ļ',
  M: 'Ṁ',
  N: 'Ñ',
  O: 'Ó',
  P: 'Þ',
  Q: 'Ǫ',
  R: 'Ŕ',
  S: 'Š',
  T: 'Ŧ',
  U: 'Ú',
  V: 'Ṽ',
  W: 'Ŵ',
  X: 'Ẋ',
  Y: 'Ý',
  Z: 'Ž',
};

/** Interpolations (`{{name}}`) and Trans tags (`<1>`, `</1>`, `<kbd/>`) pass through untouched. */
const PLACEHOLDER = /(\{\{[^}]+\}\}|<\/?[\w-]+\s*\/?>)/;

/** How much longer pseudo strings get: translations often run ~30% longer than English. */
export const PSEUDO_EXPANSION = 0.3;

/**
 * Accents every letter and pads the message by {@link PSEUDO_EXPANSION},
 * bracketed so truncation and concatenated strings are visible at a glance.
 */
export function pseudoMessage(message: string): string {
  let visible = 0;
  const body = message
    .split(PLACEHOLDER)
    .map((part, i) => {
      if (i % 2 === 1) return part;
      visible += part.length;
      return part.replace(/[a-z]/gi, (ch) => ACCENTED[ch] ?? ch);
    })
    .join('');
  const pad = '·'.repeat(Math.ceil(visible * PSEUDO_EXPANSION));
  return `⟦${body}${pad}⟧`;
}

/** The pseudo-locale catalogue for `catalog`. */
export function pseudoLocalize(catalog: Catalog): Catalog {
  return mapMessages(catalog, pseudoMessage);
}
