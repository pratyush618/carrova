import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import type { MessageRow } from '../../../bindings/MessageRow';
import type { SmsSend } from '../../../flows/phone/useSmsSends';
import { Icon, LinkButton } from '../../../ui';
import { cx } from '../../../ui/cx';

/**
 * One conversation, oldest first, then the texts sent from here that are
 * on their way or did not (surely) go, each saying where it stands.
 */
export function Bubbles({
  messages,
  sends,
  phone,
  onRetry,
}: {
  readonly messages: readonly MessageRow[];
  readonly sends: readonly SmsSend[];
  readonly phone: string;
  readonly onRetry: (clientId: number) => void;
}) {
  const { t } = useTranslation();
  const list = useRef<HTMLOListElement>(null);
  // The newest text is at the bottom: keep it in view as texts arrive.
  useEffect(() => {
    const el = list.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [messages.length, sends.length]);
  return (
    <ol ref={list} className="s-bubbles">
      {messages.map((m, i) => (
        <li
          // A provisional text has id 0 until the phone stores it.
          key={m.id === 0 ? `p${String(i)}` : m.id}
          className={cx('s-bub', m.outgoing ? 's-bub--out' : 's-bub--in')}
        >
          {m.text}
        </li>
      ))}
      {sends.map((s) => (
        <li key={s.clientId} className="s-bub-wrap">
          <p
            className={cx(
              's-bub',
              's-bub--out',
              s.state === 'sending' ? 's-bub--pending' : 's-bub--fail',
            )}
          >
            {s.text}
          </p>
          <p className={cx('s-bubmeta', s.state !== 'sending' && 's-bubmeta--fail')} role="status">
            {s.state !== 'sending' && <Icon name="warn" size="sm" />}
            {t(`ecosystem.messages.${s.state}`, { name: phone })}
            {(s.state === 'no_signal' || s.state === 'failed') && (
              <LinkButton
                onClick={() => {
                  onRetry(s.clientId);
                }}
              >
                {t('ecosystem.messages.retry')}
              </LinkButton>
            )}
          </p>
        </li>
      ))}
    </ol>
  );
}
