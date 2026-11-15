import { useTranslation } from 'react-i18next';
import type { ClipNote } from '../../app/state/appState';
import { Toast } from '../../ui';

/**
 * A clip note: why this computer's clipboard stayed here (a password, or
 * too big), or, if the user asked for a note, where an arrived one came from.
 */
export function ClipToast({
  note,
  device,
  onDismiss,
}: {
  readonly note: ClipNote;
  /** The name of the device a received clip came from. */
  readonly device: string;
  readonly onDismiss: () => void;
}) {
  const { t } = useTranslation();
  if (note.kind === 'received')
    return (
      <Toast icon="copy" onDismiss={onDismiss}>
        {t('ecosystem.clip.received', { name: device })}
      </Toast>
    );
  return (
    <Toast icon={note.reason === 'sensitive' ? 'lock' : 'copy'} onDismiss={onDismiss}>
      {t(`ecosystem.clip.${note.reason}`)}
    </Toast>
  );
}
