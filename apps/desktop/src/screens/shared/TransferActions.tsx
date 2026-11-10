import { useTranslation } from 'react-i18next';
import { ActionBar, Button } from '../../ui';
import type { TransferIntents, TransferModel, TransferPhase } from './transfer';

/** The bar's note per phase; the engine retries a lost link by itself, so that says so. */
const NOTE = {
  moving: 'transfer.awake',
  paused: null,
  held: null,
  reconnecting: 'transfer.resumesItself',
  lost: 'transfer.resumeWhenBack',
  applying: 'transfer.applyNote',
} as const satisfies Record<TransferPhase, string | null>;

/**
 * Cancel is always offered until the final apply, which can't stop half-way:
 * then the bar says it finishes or undoes itself. While paused for space, the
 * override sits next to Cancel.
 */
export function TransferActions({
  model,
  on,
}: {
  readonly model: TransferModel;
  readonly on: TransferIntents;
}) {
  const { t } = useTranslation();
  const applying = model.phase === 'applying';
  const note = NOTE[model.phase];
  return (
    <ActionBar
      start={
        !applying && (
          <>
            <Button kind="quiet-danger" onClick={on.onCancel}>
              {t('action.cancel')}
            </Button>
            {model.phase === 'paused' && on.onIgnoreSpace && (
              <Button kind="quiet" onClick={on.onIgnoreSpace}>
                {t('transfer.ignoreSpace')}
              </Button>
            )}
          </>
        )
      }
      note={note === null ? '' : t(note)}
      end={
        <>
          {model.phase === 'moving' && on.onPause && (
            <Button icon="pause" onClick={on.onPause}>
              {t('transfer.pause')}
            </Button>
          )}
          {model.phase === 'reconnecting' && on.onTryNow && (
            <Button onClick={on.onTryNow}>{t('transfer.tryNow')}</Button>
          )}
          {model.phase === 'lost' && on.onResumeSession && (
            <Button kind="primary" icon="play" onClick={on.onResumeSession}>
              {t('transfer.resume')}
            </Button>
          )}
          {model.phase === 'held' && on.onResume && (
            <Button kind="primary" icon="play" onClick={on.onResume}>
              {t('transfer.resume')}
            </Button>
          )}
          {applying && on.onSeeArrived && (
            <Button kind="primary" onClick={on.onSeeArrived}>
              {t('transfer.seeArrived')}
            </Button>
          )}
        </>
      }
    />
  );
}
