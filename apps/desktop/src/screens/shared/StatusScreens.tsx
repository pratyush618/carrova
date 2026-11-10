import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import type { BridgeError } from '../../engine/types';
import { ActionBar, Button, Empty, FlowLayout, Head, Status } from '../../ui';

/** A step waiting on the engine: what's happening, in words, and the way out. */
export function BusyScreen({
  title,
  status,
  onCancel,
}: {
  readonly title: string;
  readonly status: string;
  /** Omitted when the work can't be stopped (it finishes or undoes itself). */
  readonly onCancel?: () => void;
}) {
  const { t } = useTranslation();
  return (
    <FlowLayout
      actions={
        <ActionBar start={onCancel && <Button onClick={onCancel}>{t('action.cancel')}</Button>} />
      }
    >
      <Head title={title} />
      <Status>{status}</Status>
    </FlowLayout>
  );
}

/** A step that failed: what happened, the engine's words, and one recovery. */
export function FailedScreen({
  error,
  onRetry,
  onHome,
}: {
  readonly error: BridgeError;
  readonly onRetry: () => void;
  readonly onHome: () => void;
}) {
  const { t } = useTranslation();
  const text = errorText(error, t);
  return (
    <FlowLayout
      actions={<ActionBar start={<Button onClick={onHome}>{t('action.backHome')}</Button>} />}
    >
      <Empty
        art="search"
        tone="danger"
        title={text.title}
        body={text.body}
        action={
          <Button kind="primary" onClick={onRetry}>
            {t('action.tryAgain')}
          </Button>
        }
      />
    </FlowLayout>
  );
}
