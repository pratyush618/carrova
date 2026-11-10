import { useTranslation } from 'react-i18next';
import { ActionBar, Banner, Button, FlowLayout, Handshake, Head, Status } from '../../ui';
import { TransferActions } from '../shared/TransferActions';
import { TransferBody } from '../shared/TransferBody';
import type { TransferIntents } from '../shared/transfer';
import type { LiveModel } from './model';

export interface SendLiveScreenProps {
  readonly model: LiveModel;
  readonly on: TransferIntents;
}

/** One live screen: the handshake lands, then becomes progress when they approve. */
export function SendLiveScreen({ model, on }: SendLiveScreenProps) {
  const { t } = useTranslation();
  if (model.status === 'transfer') {
    return (
      <FlowLayout actions={<TransferActions model={model.transfer} on={on} />}>
        <TransferBody model={model.transfer} on={on} />
      </FlowLayout>
    );
  }
  return (
    <FlowLayout
      actions={
        <ActionBar
          start={
            <Button kind="danger" onClick={on.onCancel}>
              {t('action.cancel')}
            </Button>
          }
        />
      }
    >
      <Head
        title={t('send.live.connected', { peer: model.peer })}
        lede={t('send.live.reviewingLede')}
      />
      <Handshake here={model.here} there={model.peer} linked />
      <Status>{t('send.live.waitingApproval')}</Status>
      <Banner
        tone="accent"
        title={t('send.live.replaced', { count: model.replaced })}
        body={t('send.live.replacedBody')}
      />
    </FlowLayout>
  );
}
