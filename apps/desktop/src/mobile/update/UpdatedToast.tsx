import { useTranslation } from 'react-i18next';
import { useHost } from '../../engine/host';
import { Button, Toast } from '../../ui';
import { releaseNotesUrl } from './model';

/** "Updated to X", once after an update, with its release notes a tap away. */
export function UpdatedToast({
  version,
  onDismiss,
}: {
  readonly version: string;
  readonly onDismiss: () => void;
}) {
  const { t } = useTranslation();
  const host = useHost();
  return (
    <Toast
      onDismiss={onDismiss}
      action={
        <Button
          kind="quiet"
          size="sm"
          onClick={() => {
            onDismiss();
            void host.openUrl?.(releaseNotesUrl(version));
          }}
        >
          {t('mobile.update.whatsNew')}
        </Button>
      }
    >
      {t('mobile.update.updated', { version })}
    </Toast>
  );
}
