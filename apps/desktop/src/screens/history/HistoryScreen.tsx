import { useTranslation } from 'react-i18next';
import { isNearDay } from '../../i18n/format';
import { useFormat } from '../../i18n/useFormat';
import { Badge, Button, Empty, Head, HubLayout, Icon, MoveCard, OsBadge } from '../../ui';
import type { HistoryEntry, HistoryIntents, HistoryModel } from './model';
import { MoveDetail } from './MoveDetail';
import { groupMoves, KIND_ICON, moveSub, moveTag, moveTitle } from './moveWords';
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
  const tag = moveTag(entry, t);
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
      tag={
        tag && (
          <Badge tone={tag.tone} {...(tag.icon && { icon: tag.icon })}>
            {tag.label}
          </Badge>
        )
      }
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
  const groups = groupMoves(model.entries, (e) =>
    isNearDay(e.when, model.now) ? format.day(e.when, model.now, true) : month.format(e.when),
  );
  return (
    <HubLayout variant="flush">
      <div className="s-hist">
        <div className="s-hist__list">
          <Head title={t('history.title')} lede={t('history.lede')} />
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
