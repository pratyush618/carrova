import type { TFunction } from 'i18next';
import type { RequestSummary } from '../../engine/types';
import type { Formatters } from '../../i18n/useFormat';

/** `12 apps · 340 MB · includes keys`: what a move request carries before Review (counts only). */
export function countsText(summary: RequestSummary, t: TFunction, format: Formatters): string {
  const values = {
    apps: t('count.apps', { count: summary.apps }),
    size: format.bytes(summary.bytes),
  };
  return t(summary.has_sensitive ? 'request.countsKeys' : 'request.counts', values);
}
