import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '../../ui';

/** Forgetting a session drops its undo point for good, so it is asked once, plainly. */
export function ForgetDialog({
  undoable,
  on,
}: {
  readonly undoable: boolean;
  readonly on: { readonly onKeep: () => void; readonly onForget: () => void };
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      tone="warning"
      icon="warn"
      title={t('history.forget.title')}
      onDismiss={on.onKeep}
      foot={
        <>
          <Button kind="primary" data-initial-focus onClick={on.onKeep}>
            {t('history.forget.keep')}
          </Button>
          <Button kind="danger" onClick={on.onForget}>
            {t('history.forget.go')}
          </Button>
        </>
      }
    >
      <p>{undoable ? t('history.forget.bodyUndo') : t('history.forget.body')}</p>
    </Dialog>
  );
}
