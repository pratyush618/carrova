import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  Banner,
  HubLayout,
  LockLine,
  Muted,
  Search,
  Segmented,
  type SegmentOption,
} from '../../ui';
import { AppCard } from './AppCard';
import type { CatalogIntents, CatalogModel, TierFilter } from './model';
import { visibleEntries } from './visibleEntries';
import './apps.css';

const FILTERS: readonly TierFilter[] = ['all', 'verified', 'community'];

/** The catalogue: a grid of known apps, tier as a badge, signature status as one lock line. */
export function AppsScreen({
  model,
  on,
}: {
  readonly model: CatalogModel;
  readonly on: CatalogIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const filters: readonly SegmentOption<TierFilter>[] = FILTERS.map((value) => ({
    value,
    label: t(`apps.filter.${value}`),
  }));
  const shown = visibleEntries(model.entries, model.filter, model.query);
  return (
    <HubLayout
      tools={
        <Segmented
          label={t('apps.show')}
          options={filters}
          value={model.filter}
          onChange={on.onFilter}
        />
      }
    >
      <Search
        label={t('apps.search')}
        placeholder={t('apps.searchPlaceholder', { count: model.entries.length })}
        value={model.query}
        onChange={on.onQuery}
      />
      {model.bundle.updated !== null ? (
        <LockLine>
          {t('apps.updated', { day: format.day(model.bundle.updated, model.now) })}
        </LockLine>
      ) : (
        <Banner tone="warning" icon="warn" title={t('apps.unsigned')} />
      )}
      {model.filter === 'community' && (
        <Banner
          tone="warning"
          icon="info"
          title={t('apps.communityTitle')}
          body={t('apps.communityBody')}
        />
      )}
      {shown.length === 0 ? (
        <Muted>{t('apps.noMatch', { query: model.query })}</Muted>
      ) : (
        <ul className="s-grid" aria-label={t('apps.grid')}>
          {shown.map((entry) => (
            <AppCard
              key={entry.id}
              entry={entry}
              onOpen={() => {
                on.onOpen(entry.id);
              }}
            />
          ))}
        </ul>
      )}
    </HubLayout>
  );
}
