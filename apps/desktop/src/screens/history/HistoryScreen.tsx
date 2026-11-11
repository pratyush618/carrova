import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { isNearDay } from '../../i18n/format';
import { useFormat } from '../../i18n/useFormat';
import {
  Button,
  Empty,
  HubLayout,
  Icon,
  Muted,
  MoveCard,
  OsBadge,
  Search,
  Segmented,
} from '../../ui';
import type { HistoryEntry, HistoryIntents, HistoryModel } from './model';
import { MoveDetail } from './MoveDetail';
import { MoveTagBadge } from './MoveTagBadge';
import {
  groupMoves,
  KIND_ICON,
  MOVE_FILTERS,
  type MoveFilter,
  moveSub,
  moveTag,
  moveTitle,
  visibleMoves,
} from './moveWords';
import './history.css';

function Card({
  entry,
  model,
  onSelect,
}: {
  readonly entry: HistoryEntry;
  readonly model: HistoryModel;
  readonly onSelect: () => void;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const tag = moveTag(entry);
  return (
    <MoveCard
      route={
        <>
          <Icon name={KIND_ICON[entry.kind]} size="sm" />
          {moveTitle(entry, t)}
          {entry.peerOs !== null && (
            <OsBadge os={entry.peerOs} label={t(`settings.about.os.${entry.peerOs}`)} size="sm" />
          )}
        </>
      }
      when={isNearDay(entry.when, model.now) ? format.time(entry.when) : format.date(entry.when)}
      sub={moveSub(entry, model.now, t, format)}
      tag={tag && <MoveTagBadge tag={tag} />}
      current={model.selected?.entry.id === entry.id}
      undone={entry.kind === 'undone'}
      onSelect={onSelect}
    />
  );
}

/** Every move as a card on a timeline; the selected one opens its record beside the list. */
export function HistoryScreen({
  model,
  on,
}: {
  readonly model: HistoryModel;
  readonly on: HistoryIntents;
}) {
  const { t, i18n } = useTranslation();
  const format = useFormat();
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<MoveFilter>('all');
  if (model.entries.length === 0 || model.selected === null) {
    return (
      <HubLayout>
        <Empty
          art="history"
          title={t('history.emptyTitle')}
          body={t('history.emptyBody')}
          action={
            <Button kind="primary" onClick={on.onReceive}>
              {t('history.emptyAction')}
            </Button>
          }
        />
      </HubLayout>
    );
  }
  const month = new Intl.DateTimeFormat(i18n.language, { month: 'long', year: 'numeric' });
  const groups = groupMoves(visibleMoves(model.entries, filter, query), (e) =>
    isNearDay(e.when, model.now) ? format.day(e.when, model.now, true) : month.format(e.when),
  );
  return (
    <HubLayout
      variant="flush"
      tools={
        <>
          <Search
            label={t('history.search')}
            placeholder={t('history.search')}
            value={query}
            onChange={setQuery}
          />
          <Segmented
            label={t('history.show')}
            options={MOVE_FILTERS.map((value) => ({ value, label: t(`history.filter.${value}`) }))}
            value={filter}
            onChange={setFilter}
          />
        </>
      }
    >
      <div className="s-hist">
        <div className="s-hist__list">
          {groups.length === 0 && <Muted>{t('history.noMatch')}</Muted>}
          {groups.map((group) => (
            <section key={group.label} aria-label={group.label}>
              <h2 className="s-hist__day">{group.label}</h2>
              {group.items.map((entry) => (
                <Card
                  key={entry.id}
                  entry={entry}
                  model={model}
                  onSelect={() => {
                    on.onSelect(entry.id);
                  }}
                />
              ))}
            </section>
          ))}
        </div>
        <aside className="s-hist__detail" aria-label={t('history.detail')}>
          <MoveDetail model={model.selected} now={model.now} on={on} />
        </aside>
      </div>
    </HubLayout>
  );
}
