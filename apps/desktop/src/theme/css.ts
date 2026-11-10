import { colorTokens, effectTokens, scales } from './tokens.ts';

/** Which column of the token table to emit. */
export type ThemeName = 'light' | 'dark';

const declarations = (entries: readonly (readonly [string, string])[]): string =>
  entries.map(([name, value]) => `  --${name}: ${value};`).join('\n');

/** Custom-property declarations for one theme (colours + effects). */
export function themeDeclarations(theme: ThemeName): string {
  const colors = Object.entries(colorTokens).map(([name, t]) => [name, t[theme]] as const);
  const effects = Object.entries(effectTokens).map(([name, t]) => [name, t[theme]] as const);
  return `  color-scheme: ${theme};\n${declarations([...colors, ...effects])}`;
}

/** Custom-property declarations for the theme-independent scales. */
export function scaleDeclarations(): string {
  return declarations(scales.flatMap((scale) => Object.entries(scale)));
}

/**
 * The whole token stylesheet. Light is the base; dark applies when the user
 * picks it (`data-theme="dark"`) or when the OS prefers dark and the user has
 * not forced light. Emitted at build time by the Vite tokens plugin, so it is
 * a normal same-origin stylesheet under the app's CSP.
 */
export function buildTokenCss(): string {
  const dark = themeDeclarations('dark');
  return [
    `:root {\n${scaleDeclarations()}\n${themeDeclarations('light')}\n}`,
    `:root[data-theme='dark'] {\n${dark}\n}`,
    `@media (prefers-color-scheme: dark) {\n:root:not([data-theme='light']) {\n${dark}\n}\n}`,
  ].join('\n\n');
}
