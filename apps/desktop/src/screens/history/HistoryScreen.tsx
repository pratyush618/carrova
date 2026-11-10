import { useTranslation } from 'react-i18next';
import { isNearDay } from '../../i18n/format';
import { useFormat } from '../../i18n/useFormat';
import {
  Badge,
  Button,
  Empty,
  HubLayout,
  Icon,
  LinkRow,
  List,
  PageHead,
  type IconName,
} from '../../ui';
import type { HistoryEntry, HistoryIntents, HistoryModel, MoveKind } from './model';
import { moveTitle } from './moveTitle';

const KIND_ICON: Record<MoveKind, IconName> = {
  received: 'receive',
  sent: 'send',
  saved: 'file',
  opened: 'file',
  undone: 'undo',
};

function EntryRow({
  entry,
  now,
  onOpen,
}: {
  readonly entry: HistoryEntry;
  readonly now: Date;
  readonly onOpen: () => void;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const when = isNearDay(entry.when, now)
    ? t('summary.dayTime', {
        day: format.day(entry.when, now, true),
        time: format.time(entry.when),
      })
    : format.date(entry.when);
  const moved = entry.moved;
  const apps = moved === null ? '' : t('count.apps', { count: moved.apps });
  const bytes = moved?.bytes ?? null;
  const summary =
    bytes === null ? apps : t('summary.appsSize', { apps, size: format.bytes(bytes) });
  const sub = moved ? t('history.when', { when, summary }) : when;
  return (
    <LinkRow
      lead={<Icon name={KIND_ICON[entry.kind]} />}
      title={moveTitle(entry, t)}
      sub={sub}
      trail={
        <>
          {entry.undoable && <Badge tone="accent">{t('history.undoable')}</Badge>}
          {entry.resumable && <Badge tone="warning">{t('history.interrupted')}</Badge>}
          <Icon name="chev" />
        </>
      }
      onClick={onOpen}
    />
  );
}

/** Every past move, each with a way back to its details and Undo. */
export function HistoryScreen({
  model,
  on,
}: {
  readonly model: HistoryModel;
  readonly on: HistoryIntents;
}) {
  const { t } = useTranslation();
  if (model.entries.length === 0) {
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
  return (
    <HubLayout>
      <PageHead title={t('history.title')} />
      <List label={t('history.moves')}>
        {model.entries.map((entry) => (
          <EntryRow
            key={entry.id}
            entry={entry}
            now={model.now}
            onOpen={() => {
              on.onOpen(entry.id);
            }}
          />
        ))}
      </List>
    </HubLayout>
  );
}
