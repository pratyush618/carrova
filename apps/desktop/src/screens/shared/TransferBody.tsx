import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { minutesLeft } from '../../i18n/format';
import { Banner, Button, Chip, Chips, Head, LockLine, Progress } from '../../ui';
import { chipText } from './chipText';
import type { TransferIntents, TransferModel } from './transfer';

export interface TransferBodyProps {
  readonly model: TransferModel;
  readonly on: Pick<TransferIntents, 'onOpenFiles'>;
}

function useMeta(model: TransferModel): readonly [string, string] {
  const { t } = useTranslation();
  const format = useFormat();
  const ofTotal = t('summary.ofTotal', {
    done: format.bytes(model.done),
    total: format.bytes(model.total),
  });
  switch (model.phase) {
    case 'applying':
      return [
        t('transfer.arrivedTotal', { size: format.bytes(model.total) }),
        t('transfer.almostDone'),
      ];
    case 'paused':
      return [ofTotal, t('transfer.waitingSpace')];
    case 'held':
      return [ofTotal, t('transfer.heldMeta')];
    case 'reconnecting':
      return [ofTotal, t('transfer.retrying')];
    case 'moving':
      return [
        ofTotal,
        model.secondsLeft === null
          ? ''
          : t('summary.minutesLeft', { count: minutesLeft(model.secondsLeft) }),
      ];
  }
}

/** The progress body: one bar, app chips ticking off small-first, and the lock line. */
export function TransferBody({ model, on }: TransferBodyProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const meta = useMeta(model);
  const shortfall = format.bytes(model.shortfall);
  return (
    <>
      <Head
        title={t(`transfer.title.${model.phase}`)}
        lede={t(`transfer.lede.${model.phase}`, { size: shortfall })}
      />
      {model.phase === 'paused' && (
        <Banner
          tone="danger"
          title={t('transfer.spaceTitle', { size: shortfall })}
          body={t('transfer.spaceBody')}
          action={
            on.onOpenFiles && (
              <Button size="sm" onClick={on.onOpenFiles}>
                {t('transfer.openFiles')}
              </Button>
            )
          }
        />
      )}
      <Progress
        value={model.total === 0 ? 0 : model.done / model.total}
        label={t('ui.progress')}
        paused={model.phase !== 'moving' && model.phase !== 'applying'}
        meta={meta}
      />
      <Chips label={t('transfer.apps')}>
        {model.apps.map((app) => (
          <Chip
            key={app.id}
            glyph={app.glyph}
            name={app.name}
            sub={format.bytes(app.bytes)}
            state={app.state}
            stateText={chipText(app, model, t, format.bytes)}
          />
        ))}
      </Chips>
      <LockLine>
        {model.side === 'send'
          ? t('transfer.lockSend', { peer: model.peer })
          : t('transfer.lockReceive')}
      </LockLine>
    </>
  );
}
