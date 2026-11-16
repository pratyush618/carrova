import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { isPhase2, type Phase2Feature } from './model';

/**
 * What a device just linked does now, from its own settings (a device
 * linked again keeps its choices; a computer starts with everything off).
 */
export function LinkedSummary({ device }: { readonly device: LinkedDeviceView | null }) {
  const { t, i18n } = useTranslation();
  const list = new Intl.ListFormat(i18n.language, { type: 'conjunction' });
  const named = (state: 'on' | 'ask') =>
    (device?.features ?? [])
      .filter((f) => f.local === state)
      .map((f) => f.feature)
      .filter(isPhase2)
      .map((feature: Phase2Feature) => t(`ecosystem.features.${feature}.title`));
  const on = named('on');
  const ask = named('ask');
  const parts = [
    on.length > 0 && t('ecosystem.pair.summary.on', { count: on.length, list: list.format(on) }),
    ask.length > 0 &&
      t('ecosystem.pair.summary.ask', { count: ask.length, list: list.format(ask) }),
    device !== null && on.length + ask.length === 0 && t('ecosystem.pair.summary.none'),
    t('ecosystem.pair.summary.change'),
  ].filter((part) => part !== false);
  return <p>{parts.join(' ')}</p>;
}
