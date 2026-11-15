import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '../../ui';

/** Forgetting a linked device stops everything with it at once; linking again needs a new code. */
export function ForgetDialog({
  name,
  on,
}: {
  readonly name: string;
  readonly on: { readonly onCancel: () => void; readonly onForget: () => void };
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      tone="danger"
      icon="warn"
      title={t('ecosystem.forget.title', { name })}
      onDismiss={on.onCancel}
      foot={
        <>
          <Button data-initial-focus onClick={on.onCancel}>
            {t('ecosystem.forget.cancel')}
          </Button>
          <Button kind="danger" onClick={on.onForget}>
            {t('ecosystem.forget.confirm')}
          </Button>
        </>
      }
    >
      <p>{t('ecosystem.forget.body')}</p>
    </Dialog>
  );
}
