import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '../../ui';
import type { NewCodeIntents, NewCodeModel } from './model';

/**
 * A wrong code was typed on another computer. The sender decides: show a new
 * code, or stop pairing. Stopping is the safe choice and has focus.
 */
export function NewCodeDialog({
  model,
  on,
}: {
  readonly model: NewCodeModel;
  readonly on: NewCodeIntents;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      tone="warning"
      icon="warn"
      title={t('send.newCode.title')}
      onDismiss={on.onStop}
      foot={
        <>
          <Button data-initial-focus onClick={on.onStop}>
            {t('send.newCode.stop')}
          </Button>
          <Button kind="primary" onClick={on.onNewCode}>
            {t('send.newCode.go')}
          </Button>
        </>
      }
    >
      <p>{t('send.newCode.body', { count: model.failures })}</p>
      <p>{t('send.newCode.remaining', { count: model.remaining })}</p>
    </Dialog>
  );
}
