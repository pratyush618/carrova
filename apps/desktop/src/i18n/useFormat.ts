import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { formatBytes, formatDate, formatDay, formatNumber, formatTime } from './format';

/** Locale-bound formatters for the current language. */
export interface Formatters {
  readonly bytes: (n: number) => string;
  readonly number: (n: number) => string;
  readonly time: (d: Date) => string;
  readonly date: (d: Date) => string;
  /** Relative day; `sentenceStart` capitalises it. */
  readonly day: (d: Date, now: Date, sentenceStart?: boolean) => string;
}

/** Formatters that follow the active language (Intl under the hood). */
export function useFormat(): Formatters {
  const { i18n } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  return useMemo(
    () => ({
      bytes: (n) => formatBytes(n, locale),
      number: (n) => formatNumber(n, locale),
      time: (d) => formatTime(d, locale),
      date: (d) => formatDate(d, locale),
      day: (d, now, sentenceStart) => formatDay(d, now, locale, sentenceStart),
    }),
    [locale],
  );
}
