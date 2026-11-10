import type { Retention } from '../../engine/types';

/** Undo periods offered besides "until removed" (the design's 7/30/90 scale). */
const DAYS = [7, 30, 90] as const;

/** A retention as one select value: its name, or its days. */
export const retentionValue = (retention: Retention): string =>
  typeof retention === 'string' ? retention : String(retention.keep_for.days);

/** The retention a select value stands for (inverse of {@link retentionValue}). */
export const retentionOf = (value: string): Retention =>
  value === 'until_dismissed' || value === 'discard_on_success'
    ? value
    : { keep_for: { days: Number(value) } };

/** The retention choices, including `current` when it is a period not on the scale. */
export function retentionChoices(current: Retention): Retention[] {
  const days =
    typeof current === 'string' ? [...DAYS] : [...new Set([...DAYS, current.keep_for.days])];
  return [
    'until_dismissed',
    ...days.sort((a, b) => a - b).map((n) => ({ keep_for: { days: n } })),
    'discard_on_success',
  ];
}
