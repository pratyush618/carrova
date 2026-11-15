import { useTranslation } from 'react-i18next';
import type { Transfer } from '../../app/state/appState';
import { useFormat } from '../../i18n/useFormat';
import { Progress, Toast } from '../../ui';

/** A file moving with a linked device: progress, then saved or why it stopped. */
export function TransferToast({
  device,
  transfer,
  onDismiss,
}: {
  /** The other device's name. */
  readonly device: string;
  readonly transfer: Transfer;
  readonly onDismiss: () => void;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  // The name arrives with the saved file; until then the file has none.
  const file = transfer.file_name === '' ? t('ecosystem.transfer.aFile') : transfer.file_name;
  const { state } = transfer;
  switch (state.kind) {
    case 'active':
      return (
        <Toast icon="receive" onDismiss={onDismiss}>
          <Progress
            size="sm"
            label={t('ecosystem.transfer.label', { name: device })}
            value={transfer.total === 0 ? 0 : transfer.done / transfer.total}
            meta={[
              t('ecosystem.transfer.with', { file, name: device }),
              `${format.bytes(transfer.done)} / ${format.bytes(transfer.total)}`,
            ]}
          />
        </Toast>
      );
    case 'done':
      return <Toast onDismiss={onDismiss}>{t('ecosystem.transfer.saved', { file })}</Toast>;
    case 'failed':
      return (
        <Toast icon="warn" onDismiss={onDismiss}>
          {t(`ecosystem.transfer.${state.reason}`, { name: device, file })}
        </Toast>
      );
  }
}
