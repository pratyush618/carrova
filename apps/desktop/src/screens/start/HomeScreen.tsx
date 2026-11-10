import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Button, ChoiceTile, HubLayout, Icon, LinkButton } from '../../ui';
import type { HomeIntents, HomeModel, LastMove } from './model';
import './start.css';

export interface HomeScreenProps {
  readonly model: HomeModel;
  readonly on: HomeIntents;
}

function LastMoveLine({
  last,
  now,
  onUndo,
}: {
  readonly last: LastMove;
  readonly now: Date;
  readonly onUndo: () => void;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const values = {
    peer: last.peer,
    day: format.day(last.when, now),
    apps: t('count.apps', { count: last.apps }),
  };
  return (
    <p className="s-home__last">
      <Icon name="history" size="sm" />
      {last.direction === 'received'
        ? t('start.home.lastReceived', values)
        : t('start.home.lastSent', values)}
      {last.undoable && (
        <Button kind="quiet" size="sm" onClick={onUndo}>
          {t('start.home.undo')}
        </Button>
      )}
    </p>
  );
}

/** Two verbs, nothing else competes. The last move is one line, with Undo. */
export function HomeScreen({ model, on }: HomeScreenProps) {
  const { t } = useTranslation();
  return (
    <HubLayout variant="center">
      <h1 className="p-title" tabIndex={-1} data-autofocus>
        {t('start.home.title')}
      </h1>
      <div className="s-home__choices">
        <ChoiceTile
          icon="send"
          title={t('start.home.send.title')}
          sub={t('start.home.send.sub')}
          onClick={on.onSend}
        />
        <ChoiceTile
          icon="receive"
          title={t('start.home.receive.title')}
          sub={t('start.home.receive.sub')}
          onClick={on.onReceive}
        />
      </div>
      <LinkButton onClick={on.onUseFile}>{t('start.home.useFile')}</LinkButton>
      {model.last && <LastMoveLine last={model.last} now={model.now} onUndo={on.onUndoLast} />}
    </HubLayout>
  );
}
