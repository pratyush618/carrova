import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { MessageRow } from '../../../bindings/MessageRow';
import type { SimChoice } from '../../../bindings/SimChoice';
import type { ThreadRow } from '../../../bindings/ThreadRow';
import type { ThreadView } from '../../../bindings/ThreadView';
import type { SmsSend } from '../../../flows/phone/useSmsSends';
import { Banner, LinkButton } from '../../../ui';
import { Bubbles } from './Bubbles';
import { Composer } from './Composer';
import { initials } from './ThreadList';

/** Whether `b` is `a` again: the same stored text, or the phone's stored copy of a provisional one. */
const sameText = (a: MessageRow, b: MessageRow): boolean =>
  a.provisional || b.provisional
    ? a.text === b.text && Math.abs(a.when_ms - b.when_ms) <= 60_000
    : a.id === b.id;

/**
 * The newest received text once one arrives while `thread` is open, for a
 * polite live region: never those already there when it loaded.
 */
function useArrived(thread: ThreadView | null): string {
  const [news, setNews] = useState('');
  // The newest text as last loaded: `undefined` until loaded; `null`: none yet.
  const tail = useRef<MessageRow | null | undefined>(undefined);
  useEffect(() => {
    if (thread === null) return;
    const was = tail.current;
    tail.current = thread.messages.at(-1) ?? null;
    if (was === undefined) return;
    // Older pages load above that text, arrivals below it.
    const from = was === null ? 0 : thread.messages.findLastIndex((m) => sameText(was, m)) + 1;
    const last = thread.messages.slice(from).findLast((m) => !m.outgoing);
    if (last !== undefined) setNews(last.text);
  }, [thread]);
  return news;
}

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
  sims,
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
  readonly sims: readonly SimChoice[] | null;
  readonly onSend: (to: string, text: string, sim: number | null) => void;
  readonly onRetry: (clientId: number) => void;
  /** Load the next page of older texts. */
  readonly onOlder: () => void;
}) {
  const { t } = useTranslation();
  const arrived = useArrived(thread);
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
      <p className="p-vh" aria-live="polite">
        {arrived}
      </p>
      <Bubbles messages={thread?.messages ?? []} sends={sends} phone={phone} onRetry={onRetry} />
      <Composer
        phone={phone}
        away={away}
        sims={sims}
        preferredSim={row.sim}
        onSend={(text, sim) => {
          onSend(row.address, text, sim);
        }}
      />
    </section>
  );
}
