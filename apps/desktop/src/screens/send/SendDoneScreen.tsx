import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { ActionBar, Arrival, Button, FlowCenter, FlowLayout, Head } from '../../ui';
import type { DeliveredIntents, DeliveredModel } from './model';

/** Completion is a moment, then what's true: nothing was removed here. */
export function SendDoneScreen({
  model,
  on,
}: {
  readonly model: DeliveredModel;
  readonly on: DeliveredIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <FlowLayout
      actions={
        <ActionBar
          start={<Button onClick={on.onDetails}>{t('action.seeDetails')}</Button>}
          end={
            <Button kind="primary" onClick={on.onDone}>
              {t('action.done')}
            </Button>
          }
        />
      }
    >
      <FlowCenter>
        <Arrival
          word={t('send.done.stamp')}
          meta={t('summary.appsSize', {
            apps: t('count.apps', { count: model.apps }),
            size: format.bytes(model.bytes),
          })}
        />
        <Head
          center
          title={t('send.done.title')}
          lede={t('send.done.lede', { peer: model.peer })}
        />
      </FlowCenter>
    </FlowLayout>
  );
}
