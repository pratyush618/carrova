import type { Plugin } from 'vite';
import { buildTokenCss } from './css.ts';

/** Import this id once (in `main.tsx`) to get the generated token stylesheet. */
export const TOKENS_MODULE_ID = 'virtual:carrova-tokens.css';
const RESOLVED_ID = `\0${TOKENS_MODULE_ID}`;

/**
 * Emits `tokens.ts` as a real stylesheet at build time: no inline `<style>`,
 * so the strict CSP (`style-src 'self'`) holds, and no second copy of the
 * token table to drift.
 */
export function tokensPlugin(): Plugin {
  return {
    name: 'carrova-tokens',
    resolveId: (id) => (id === TOKENS_MODULE_ID ? RESOLVED_ID : undefined),
    load: (id) => (id === RESOLVED_ID ? buildTokenCss() : undefined),
  };
}
