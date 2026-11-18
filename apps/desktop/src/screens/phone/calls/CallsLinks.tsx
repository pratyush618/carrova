import { useTranslation } from 'react-i18next';
import { Button, Row } from '../../../ui';

/** Under a phone's Calls row while calls are on: its call log, and the optional audio setup. */
export function CallsLinks({
  onCalls,
  onCallAudio,
}: {
  readonly onCalls: () => void;
  readonly onCallAudio?: (() => void) | undefined;
}) {
  const { t } = useTranslation();
  return (
    <>
      <Row
        nested
        title={t('calls.log.title')}
        trail={
          <Button size="sm" icon="call" onClick={onCalls}>
            {t('calls.device.open')}
          </Button>
        }
      />
      {onCallAudio && (
        <Row
          nested
          title={t('calls.device.audio')}
          trail={
            <Button size="sm" icon="mic" onClick={onCallAudio}>
              {t('calls.device.setUp')}
            </Button>
          }
        />
      )}
    </>
  );
}
