import type { ClipSkip } from '../../engine/types';
import { useTranslation } from 'react-i18next';
import { Toast } from '../../ui';

/** Why this computer's clipboard stayed here (a password, or too big). Copies that do go say nothing. */
export function ClipToast({
  reason,
  onDismiss,
}: {
  readonly reason: ClipSkip;
  readonly onDismiss: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Toast icon={reason === 'sensitive' ? 'lock' : 'copy'} onDismiss={onDismiss}>
      {t(`ecosystem.clip.${reason}`)}
    </Toast>
  );
}
