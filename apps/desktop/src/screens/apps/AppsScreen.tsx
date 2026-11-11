import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { categoryName } from '../../screens/send/FoundSelection';
import {
  Banner,
  HubLayout,
  LockLine,
  Muted,
  Search,
  Segmented,
  type SegmentOption,
} from '../../ui';
import { AppDetail } from './AppDetail';
import { AppRow } from './AppRow';
import type { CatalogIntents, CatalogModel, TierFilter } from './model';
import './apps.css';

const FILTERS: readonly TierFilter[] = ['all', 'verified', 'community'];

/** The catalogue: apps by category on the left, the selected app's recipe beside them. */
export function AppsScreen({
  model,
  on,
}: {
  readonly model: CatalogModel;
  readonly on: CatalogIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const count = model.groups.reduce((sum, g) => sum + g.total, 0);
  const filters: readonly SegmentOption<TierFilter>[] = FILTERS.map((value) => ({
    value,
    label: t(`apps.filter.${value}`),
  }));
  return (
    <HubLayout
      variant="flush"
      tools={
        <>
          <Search
            label={t('apps.search')}
            placeholder={t('apps.searchPlaceholder', { count })}
            value={model.query}
            onChange={on.onQuery}
          />
          <Segmented
            label={t('apps.show')}
            options={filters}
            value={model.filter}
            onChange={on.onFilter}
          />
        </>
      }
    >
      <div className="s-apps">
        <div className="s-apps__list">
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
          {model.groups.length === 0 ? (
            <Muted>{t('apps.noMatch', { query: model.query })}</Muted>
          ) : (
            <ul className="s-apps__groups" aria-label={t('apps.list')}>
              {model.groups.map((group) => (
                <li key={group.category}>
                  <section className="s-cat">
                    <h2 className="s-cat__head">
                      {categoryName(group.category, t)}
                      <span>{group.total}</span>
                    </h2>
                    <ul>
                      {group.shown.map((entry) => (
                        <AppRow
                          key={entry.id}
                          entry={entry}
                          query={model.query}
                          selected={model.selected?.id === entry.id}
                          onSelect={() => {
                            on.onSelect(entry.id);
                          }}
                        />
                      ))}
                    </ul>
                    {group.shown.length < group.total && (
                      <button
                        type="button"
                        className="s-cat__more"
                        onClick={() => {
                          on.onShowAll(group.category);
                        }}
                      >
                        {t('apps.showAll')}
                      </button>
                    )}
                  </section>
                </li>
              ))}
            </ul>
          )}
        </div>
        <aside className="s-apps__detail" aria-label={t('apps.detail')}>
          <AppDetail app={model.selected} />
        </aside>
      </div>
    </HubLayout>
  );
}
