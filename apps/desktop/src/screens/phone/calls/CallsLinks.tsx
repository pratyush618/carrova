import { useTranslation } from 'react-i18next';
import { Button, Row } from '../../../ui';

/** Under a phone's Calls row while calls are on: its call log. */
export function CallsLinks({ onCalls }: { readonly onCalls: () => void }) {
  const { t } = useTranslation();
  return (
    <Row
      nested
      title={t('calls.log.title')}
      trail={
        <Button size="sm" icon="call" onClick={onCalls}>
          {t('calls.device.open')}
        </Button>
      }
    />
  );
}
