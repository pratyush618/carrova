import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { ActionBar, Button, FlowCenter, FlowLayout, Head, Note, Seal } from '../../ui';
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
        <Seal ring={t('transfer.delivered.ring')} label={t('transfer.delivered.sealLabel')} />
        <Head
          hero
          center
          title={t('send.done.title')}
          lede={t('send.done.lede', { peer: model.peer })}
        />
        <Note center>
          {t('summary.appsSize', {
            apps: t('count.apps', { count: model.apps }),
            size: format.bytes(model.bytes),
          })}
        </Note>
      </FlowCenter>
    </FlowLayout>
  );
}
