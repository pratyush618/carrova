/**
 * Design tokens: the single source of truth for every colour, size and
 * duration in the UI. `css.ts` turns this module into CSS custom properties;
 * components only ever reference `var(--…)`.
 *
 * Ported from the approved mockup (`docs/mockups/gui.html`, README table).
 * Dark follows Material / Primer / HIG practice: near-black neutral base,
 * elevation by lighter surfaces, desaturated accents.
 */

/** A colour as written in the token table. Hex only, so contrast is computable. */
export type Hex = `#${string}`;

/** One semantic colour role with its light and dark values. */
export interface ColorToken {
  readonly light: Hex;
  readonly dark: Hex;
  readonly role: string;
}

/** Semantic colour roles, light and dark side by side. */
export const colorTokens = {
  backdrop: { light: '#E6E8EC', dark: '#0C0D10', role: 'Behind the window (gallery only)' },
  'surface-0': { light: '#F0F2F5', dark: '#101216', role: 'Wells: inputs, segmented track, logs' },
  'surface-1': { light: '#F7F8FA', dark: '#15171C', role: 'Window base' },
  'surface-2': { light: '#FFFFFF', dark: '#1C1E24', role: 'Cards, rows, chips' },
  'surface-3': { light: '#FFFFFF', dark: '#23262D', role: 'Sheets, dialogs' },
  'text-primary': { light: '#15181D', dark: '#E4E6EA', role: 'Body, headings' },
  'text-secondary': { light: '#3E4552', dark: '#B0B4BC', role: 'Supporting copy' },
  'text-muted': { light: '#5E6672', dark: '#8E939D', role: 'Metadata, hints' },
  border: { light: '#E3E6EB', dark: '#2C2F35', role: 'Decorative dividers' },
  'border-strong': { light: '#CDD2DA', dark: '#3C4048', role: 'Card and window edges' },
  control: { light: '#858C98', dark: '#6E747F', role: 'Edges of inputs, checkboxes, toggles' },
  accent: { light: '#3346D3', dark: '#A4ADEB', role: 'The only "act" colour' },
  'accent-hover': { light: '#2836B0', dark: '#BAC1F2', role: 'Accent under the pointer' },
  'accent-on': { light: '#FFFFFF', dark: '#151830', role: 'Text on the accent' },
  'accent-soft': { light: '#EEF0FD', dark: '#262A45', role: 'Selection, info banners' },
  warning: { light: '#8A5A00', dark: '#DDB76A', role: 'Sensitive items' },
  'warning-soft': { light: '#FFF4DC', dark: '#2A2519', role: 'Sensitive fill' },
  danger: { light: '#B42318', dark: '#EC9C92', role: 'Destructive, failed' },
  'danger-soft': { light: '#FDECEA', dark: '#2F1F1E', role: 'Destructive fill' },
  success: { light: '#1F7A4D', dark: '#82C9A5', role: 'Arrived, verified, encrypted' },
  'success-soft': { light: '#E8F5EE', dark: '#1A2B23', role: 'Success fill' },
  focus: { light: '#3346D3', dark: '#A4ADEB', role: 'Focus ring' },
  flap: { light: '#1B1E24', dark: '#2A2D35', role: 'Pairing-code tiles' },
  'flap-ink': { light: '#F7F8FA', dark: '#E4E6EA', role: 'Pairing-code letters' },
  'flap-seam': { light: '#0E1013', dark: '#15171C', role: 'Split line across a tile' },
  'flap-edge': { light: '#1B1E24', dark: '#3C4048', role: 'Tile outline (invisible in light)' },
  knob: { light: '#FFFFFF', dark: '#E4E6EA', role: 'Toggle knob' },
} as const satisfies Record<string, ColorToken>;

export type ColorTokenName = keyof typeof colorTokens;

/** Theme-dependent effects that are not plain colours (alpha, shadows). */
export const effectTokens = {
  scrim: { light: 'rgb(21 24 29 / .32)', dark: 'rgb(8 9 11 / .6)' },
  // Light: depth from soft shadows. Dark: depth from lighter surfaces; one
  // shadow remains only to lift overlays off the scrim.
  'e-1': { light: '0 1px 2px rgb(21 24 29 / .06)', dark: 'none' },
  'e-2': {
    light: '0 1px 2px rgb(21 24 29 / .05), 0 10px 28px -14px rgb(21 24 29 / .22)',
    dark: 'none',
  },
  'e-3': {
    light: '0 2px 6px rgb(21 24 29 / .08), 0 28px 64px -20px rgb(21 24 29 / .38)',
    dark: '0 24px 64px -16px rgb(0 0 0 / .55)',
  },
} as const satisfies Record<string, { light: string; dark: string }>;

/** WCAG 2.x minimums: 4.5 for body text, 3 for UI edges and focus. */
export type ContrastMinimum = 4.5 | 3;

/** A foreground/background pair that must meet `min` in both themes. */
export interface ContrastPair {
  readonly fg: ColorTokenName;
  readonly bg: ColorTokenName;
  readonly min: ContrastMinimum;
}

const text = (fg: ColorTokenName, bg: ColorTokenName): ContrastPair => ({ fg, bg, min: 4.5 });
const edge = (fg: ColorTokenName, bg: ColorTokenName): ContrastPair => ({ fg, bg, min: 3 });

/** Every pair the UI actually renders (mockup README, "Measured contrast"). */
export const contrastPairs: readonly ContrastPair[] = [
  text('text-primary', 'surface-1'),
  text('text-primary', 'surface-3'),
  text('text-secondary', 'surface-1'),
  text('text-secondary', 'surface-3'),
  text('text-muted', 'surface-1'),
  text('text-muted', 'surface-2'),
  text('text-muted', 'surface-3'),
  text('text-muted', 'surface-0'),
  text('accent', 'surface-1'),
  text('accent', 'surface-3'),
  text('accent-on', 'accent'),
  text('accent-on', 'accent-hover'),
  text('accent', 'accent-soft'),
  text('warning', 'warning-soft'),
  text('warning', 'surface-2'),
  text('danger', 'danger-soft'),
  text('danger', 'surface-2'),
  text('success', 'success-soft'),
  text('success', 'surface-2'),
  text('flap-ink', 'flap'),
  edge('control', 'surface-2'),
  edge('focus', 'surface-1'),
  edge('focus', 'surface-3'),
];

/** Font stacks. Atkinson Hyperlegible is bundled locally (see `fonts.ts`). */
export const fontTokens = {
  font: '"Atkinson Hyperlegible Next Variable", "Atkinson Hyperlegible Next", system-ui, -apple-system, "Segoe UI", "Noto Sans", Ubuntu, sans-serif',
  mono: '"Atkinson Hyperlegible Mono Variable", "Atkinson Hyperlegible Mono", ui-monospace, "JetBrains Mono", "Ubuntu Mono", monospace',
} as const;

/** Type scale, ratio 1.25 on a 15px base. */
export const typeScale = {
  't-sm': '13px',
  't-md': '15px',
  't-lg': '18px',
  't-xl': '22px',
  't-2xl': '28px',
  't-3xl': '36px',
  'lh-tight': '1.2',
  'lh-body': '1.55',
} as const;

/** 8-point spacing (4 only for hairline gaps). */
export const spaceScale = {
  'sp-0': '4px',
  'sp-1': '8px',
  'sp-2': '16px',
  'sp-3': '24px',
  'sp-4': '32px',
  'sp-5': '40px',
  'sp-6': '48px',
  'sp-8': '64px',
} as const;

/** Radii by hierarchy: control, card, sheet. */
export const radiusScale = {
  'r-ctl': '8px',
  'r-card': '14px',
  'r-sheet': '20px',
  'r-pill': '999px',
} as const;

/** Motion: presses, state changes, sheets/progress. Spring only where something lands. */
export const motionScale = {
  'dur-1': '120ms',
  'dur-2': '220ms',
  'dur-3': '420ms',
  ease: 'cubic-bezier(.2, .8, .2, 1)',
  'ease-spring': 'cubic-bezier(.3, 1.45, .5, 1)',
} as const;

/** Layout widths: flow column and hub page. */
export const layoutScale = {
  col: '36rem',
  page: '46rem',
} as const;

/** Theme-independent scales, in emission order. */
export const scales = [
  fontTokens,
  typeScale,
  spaceScale,
  radiusScale,
  motionScale,
  layoutScale,
] as const;
