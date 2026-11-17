import { useTranslation } from 'react-i18next';
import type { ThreadRow } from '../../../bindings/ThreadRow';
import type { ThreadView } from '../../../bindings/ThreadView';
import type { SmsSend } from '../../../flows/phone/useSmsSends';
import { Banner, LinkButton } from '../../../ui';
import { Bubbles } from './Bubbles';
import { Composer } from './Composer';
import { initials } from './ThreadList';

/**
 * The open conversation: who, its texts (still readable while the phone is
 * away; older pages on asking), and the composer.
 */
export function ThreadPane({
  row,
  thread,
  sends,
  phone,
  away,
  onSend,
  onRetry,
  onOlder,
}: {
  readonly row: ThreadRow;
  /** `null` until its page has loaded. */
  readonly thread: ThreadView | null;
  readonly sends: readonly SmsSend[];
  readonly phone: string;
  readonly away: boolean;
  readonly onSend: (to: string, text: string) => void;
  readonly onRetry: (clientId: number) => void;
  /** Load the next page of older texts. */
  readonly onOlder: () => void;
}) {
  const { t } = useTranslation();
  return (
    <section className="s-thread" aria-labelledby="s-thread-title">
      <header className="s-thread__head">
        <span className="s-av" aria-hidden="true">
          {initials(row.name)}
        </span>
        <div>
          <h2 id="s-thread-title">{row.name ?? row.address}</h2>
          <p>{t('ecosystem.messages.via', { address: row.address, name: phone })}</p>
        </div>
      </header>
      {away && (
        <Banner
          tone="accent"
          title={t('ecosystem.device.awayTitle', { name: phone })}
          body={t('ecosystem.messages.awayBody')}
        />
      )}
      {(thread?.next ?? null) !== null && (
        <p className="s-thread__older">
          <LinkButton onClick={onOlder}>{t('ecosystem.messages.olderMessages')}</LinkButton>
        </p>
      )}
      <Bubbles messages={thread?.messages ?? []} sends={sends} phone={phone} onRetry={onRetry} />
      <Composer
        phone={phone}
        away={away}
        onSend={(text) => {
          onSend(row.address, text);
        }}
      />
    </section>
  );
}
