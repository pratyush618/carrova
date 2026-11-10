import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { ActionBar, Arrival, Banner, Button, FlowCenter, FlowLayout, Head } from '../../ui';
import { LeftForYou } from './LeftForYou';
import type { ArrivedIntents, ArrivedModel } from './model';

export interface RecvDoneScreenProps {
  readonly model: ArrivedModel;
  readonly on: ArrivedIntents;
}

/** The stamp moment, then what's left as a short checklist. Undo sits next to Done. */
export function RecvDoneScreen({ model, on }: RecvDoneScreenProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const first = model.failures[0];
  return (
    <FlowLayout
      actions={
        <ActionBar
          start={
            on.onUndo && (
              <Button icon="undo" onClick={on.onUndo}>
                {t('receive.done.undo')}
              </Button>
            )
          }
          end={
            <Button kind="primary" onClick={on.onDone}>
              {t('action.done')}
            </Button>
          }
        />
      }
    >
      <FlowCenter>
        {first === undefined ? (
          <>
            <Arrival
              word={t('receive.done.stamp')}
              meta={t('summary.appsSize', {
                apps: t('count.apps', { count: model.apps }),
                size: format.bytes(model.bytes),
              })}
            />
            <Head
              center
              title={t('receive.done.title')}
              lede={t('receive.done.lede', { peer: model.peer, count: model.replaced })}
            />
          </>
        ) : (
          <>
            <Arrival
              word={t('receive.done.stampPartial')}
              meta={t('receive.done.partialMeta', { apps: model.apps, total: model.appsTotal })}
            />
            <Head
              center
              title={t('receive.done.titlePartial')}
              lede={t('receive.done.ledePartial', { what: first.what })}
            />
          </>
        )}
      </FlowCenter>
      {first !== undefined && (
        <Banner
          tone="danger"
          title={t('receive.done.failed', { count: model.failures.length })}
          body={t('receive.done.failedBody', { what: first.what, reason: first.reason })}
          action={
            on.onRetry && (
              <Button size="sm" onClick={on.onRetry}>
                {t('action.tryAgain')}
              </Button>
            )
          }
        />
      )}
      {model.tasks.length > 0 && <LeftForYou tasks={model.tasks} onCopy={on.onCopyCommand} />}
    </FlowLayout>
  );
}
