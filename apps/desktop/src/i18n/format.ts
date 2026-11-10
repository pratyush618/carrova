/** Decimal (SI) units, as Ubuntu's Files and Disks show sizes. */
const BYTE_UNITS = ['byte', 'kilobyte', 'megabyte', 'gigabyte', 'terabyte'] as const;

/** `1_200_000_000` → "1.2 GB". One decimal below 10 units, none above. */
export function formatBytes(bytes: number, locale: string): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1000 && unit < BYTE_UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return new Intl.NumberFormat(locale, {
    style: 'unit',
    unit: BYTE_UNITS[unit] ?? 'byte',
    unitDisplay: 'short',
    maximumFractionDigits: unit > 0 && value < 10 ? 1 : 0,
  }).format(value);
}

/** A plain count with grouping ("1,204"). */
export function formatNumber(n: number, locale: string): string {
  return new Intl.NumberFormat(locale).format(n);
}

/** Time of day ("14:02"). */
export function formatTime(date: Date, locale: string): string {
  return new Intl.DateTimeFormat(locale, {
    hour: '2-digit',
    minute: '2-digit',
    hourCycle: 'h23',
  }).format(date);
}

/** Short date ("2 Nov" / "Nov 2"). */
export function formatDate(date: Date, locale: string): string {
  return new Intl.DateTimeFormat(locale, { day: 'numeric', month: 'short' }).format(date);
}

const DAY_MS = 86_400_000;

const startOfDay = (d: Date): number =>
  new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();

/** Whole calendar days from `now` to `date` (negative = past). */
export function calendarDaysBetween(date: Date, now: Date): number {
  return Math.round((startOfDay(date) - startOfDay(now)) / DAY_MS);
}

/** Within a calendar day of `now`: shown as "today"/"yesterday"/"tomorrow". */
export function isNearDay(date: Date, now: Date): boolean {
  return Math.abs(calendarDaysBetween(date, now)) <= 1;
}

/**
 * "today" / "yesterday" within a day of `now`, else a short date.
 * `sentenceStart` capitalises it for the start of a line.
 */
export function formatDay(date: Date, now: Date, locale: string, sentenceStart = false): string {
  const days = calendarDaysBetween(date, now);
  if (!isNearDay(date, now)) return formatDate(date, locale);
  const word = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' }).format(days, 'day');
  return sentenceStart ? word.charAt(0).toLocaleUpperCase(locale) + word.slice(1) : word;
}

/** Rough time left ("About 2 minutes left") as a whole number of minutes, at least 1. */
export function minutesLeft(seconds: number): number {
  return Math.max(1, Math.round(seconds / 60));
}
