import { useTranslation } from 'react-i18next';
import { Button, LinkButton } from '../../../ui';
import { initials } from '../messages/ThreadList';
import './calls.css';

/**
 * A phone's call ringing here: answer (the audio comes here when it can,
 * else the phone's speaker turns on), decline, or leave it to the phone.
 */
export function IncomingCall({
  who,
  name,
  phone,
  on,
}: {
  /** The caller's name, else their number. */
  readonly who: string;
  /** The caller's contact name (`null`: unknown), for the initials. */
  readonly name: string | null;
  readonly phone: string;
  readonly on: {
    readonly onAnswer: () => void;
    readonly onDecline: () => void;
    /** Close this window; the phone keeps ringing. */
    readonly onAnswerOnPhone: () => void;
  };
}) {
  const { t } = useTranslation();
  return (
    <main className="s-call" aria-live="assertive">
      <span className="s-call__av" aria-hidden="true">
        {initials(name)}
      </span>
      <h1 className="s-call__who">{who}</h1>
      <p className="s-call__sub">{t('calls.window.calling', { phone })}</p>
      <div className="s-call__acts">
        <Button kind="danger" icon="callEnd" onClick={on.onDecline}>
          {t('calls.window.decline')}
        </Button>
        <span className="s-call__answer">
          <Button kind="primary" icon="call" onClick={on.onAnswer} data-initial-focus>
            {t('calls.window.answer')}
          </Button>
        </span>
      </div>
      <LinkButton onClick={on.onAnswerOnPhone}>{t('calls.window.onPhone')}</LinkButton>
    </main>
  );
}
