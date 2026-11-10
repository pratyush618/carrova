import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import { ActionBar, Button, Empty, FlowLayout, LinkButton, Note } from '../../ui';
import { TransferActions } from '../shared/TransferActions';
import { TransferBody } from '../shared/TransferBody';
import type { TransferIntents } from '../shared/transfer';
import type { ReceiveProgressIntents, ReceiveProgressModel } from './model';

export interface RecvProgressScreenProps {
  readonly model: ReceiveProgressModel;
  readonly on: TransferIntents & ReceiveProgressIntents;
}

/**
 * Transfer and apply on one screen. Paused and reconnecting keep everything
 * that arrived; a failed apply is undone automatically, with one recovery.
 */
export function RecvProgressScreen({ model, on }: RecvProgressScreenProps) {
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
      actions={<ActionBar start={<Button onClick={on.onHome}>{t('action.backHome')}</Button>} />}
    >
      <Empty
        art="undo"
        tone="danger"
        title={t('receive.failed.title')}
        body={t('receive.failed.body', { reason: errorText(model.error, t).body })}
        action={
          on.onRetry && (
            <Button kind="primary" onClick={on.onRetry}>
              {t('action.tryAgain')}
            </Button>
          )
        }
      />
      <Note center>
        <LinkButton onClick={on.onSeeWhat}>{t('receive.failed.seeWhat')}</LinkButton>
      </Note>
    </FlowLayout>
  );
}
