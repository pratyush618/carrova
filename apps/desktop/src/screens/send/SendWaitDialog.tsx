import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Button, Dialog, Progress, type DialogTone, type IconName } from '../../ui';
import { countsText } from '../shared/requestCounts';
import { clock, REQUEST_SECS, useSecondsLeft } from './countdown';
import type { SendWaitIntents, SendWaitModel, WaitState } from './model';

const LOOK: Readonly<Record<WaitState, { readonly icon: IconName; readonly tone: DialogTone }>> = {
  waiting: { icon: 'send', tone: 'accent' },
  reviewing: { icon: 'eye', tone: 'accent' },
  declined: { icon: 'close', tone: 'warning' },
  timeout: { icon: 'clock', tone: 'warning' },
  away: { icon: 'noNet', tone: 'warning' },
  needs_code: { icon: 'key', tone: 'warning' },
};

/** The 2-minute countdown while the request waits. */
function TimeLeft({ expiresAt }: { readonly expiresAt: number }) {
  const { t } = useTranslation();
  const seconds = useSecondsLeft(expiresAt);
  return (
    <Progress
      value={seconds / REQUEST_SECS}
      label={t('request.wait.timeLeft')}
      meta={[t('request.wait.left', { time: clock(seconds) }), null]}
    />
  );
}

/**
 * A requested send, from asking to the receiver's review: the request
 * carries counts only. Every ending says what happened and offers one way on.
 */
export function SendWaitDialog({
  model,
  on,
}: {
  readonly model: SendWaitModel;
  readonly on: SendWaitIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const name = model.peer;
  const close = (primary = false): ReactNode => (
    <Button
      {...(primary ? { kind: 'primary' } : { 'data-initial-focus': true })}
      onClick={on.onClose}
    >
      {t('ui.close')}
    </Button>
  );
  const go = (label: string, act: () => void): ReactNode => (
    <Button kind="primary" onClick={act}>
      {label}
    </Button>
  );
  const views: Readonly<Record<WaitState, { title: string; body: ReactNode; foot: ReactNode }>> = {
    waiting: {
      title: t('request.wait.waiting', { name }),
      body: (
        <>
          <p>
            {model.summary === null
              ? t('request.wait.waitingBodyPlain')
              : t('request.wait.waitingBody', { counts: countsText(model.summary, t, format) })}
          </p>
          <TimeLeft expiresAt={model.expiresAt} />
        </>
      ),
      foot: (
        <Button data-initial-focus onClick={on.onCancel}>
          {t('action.cancel')}
        </Button>
      ),
    },
    reviewing: {
      title: t('request.wait.reviewing', { name }),
      body: <p>{t('request.wait.reviewingBody')}</p>,
      foot: (
        <>
          <Button onClick={on.onCancel}>{t('request.wait.cancelMove')}</Button>
          {close(true)}
        </>
      ),
    },
    declined: {
      title: t('request.wait.declined', { name }),
      body: <p>{t('request.wait.declinedBody')}</p>,
      foot: (
        <>
          {close()}
          {go(t('request.wait.changeWhat'), on.onClose)}
        </>
      ),
    },
    timeout: {
      title: t('request.wait.timeout', { name }),
      body: <p>{t('request.wait.timeoutBody')}</p>,
      foot: (
        <>
          {close()}
          {go(t('request.wait.sendAgain'), on.onRetry)}
        </>
      ),
    },
    away: {
      title: t('request.wait.away', { name }),
      body: (
        <>
          <p>{t('request.wait.awayBody')}</p>
          <ul>
            <li>{t('request.wait.asleep')}</li>
            <li>{t('request.wait.network')}</li>
          </ul>
        </>
      ),
      foot: (
        <>
          {close()}
          {go(t('request.wait.tryAgain'), on.onRetry)}
        </>
      ),
    },
    needs_code: {
      title: t('request.wait.needsCode', { name }),
      body: <p>{t('request.wait.needsCodeBody')}</p>,
      foot: (
        <>
          {close()}
          {go(t('request.wait.useCode'), on.onUseCode)}
        </>
      ),
    },
  };
  const view = views[model.state];
  const { icon, tone } = LOOK[model.state];
  // A waiting request is stopped by Esc; a reviewing move carries on, so Esc only closes.
  const dismiss = model.state === 'waiting' ? on.onCancel : on.onClose;
  return (
    <Dialog tone={tone} icon={icon} title={view.title} onDismiss={dismiss} foot={view.foot}>
      {view.body}
    </Dialog>
  );
}
