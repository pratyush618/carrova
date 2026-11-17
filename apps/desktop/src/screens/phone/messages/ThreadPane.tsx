import { useTranslation } from 'react-i18next';
import type { SimChoice } from '../../../bindings/SimChoice';
import type { ThreadRow } from '../../../bindings/ThreadRow';
import type { ThreadView } from '../../../bindings/ThreadView';
import type { SmsSend } from '../../../flows/phone/useSmsSends';
import { Banner } from '../../../ui';
import { Bubbles } from './Bubbles';
import { Composer } from './Composer';
import { initials } from './ThreadList';

/** The open conversation: who, its texts (still readable while the phone is away), and the composer. */
export function ThreadPane({
  row,
  thread,
  sends,
  phone,
  away,
  sims,
  onSend,
  onRetry,
}: {
  readonly row: ThreadRow;
  /** `null` until its page has loaded. */
  readonly thread: ThreadView | null;
  readonly sends: readonly SmsSend[];
  readonly phone: string;
  readonly away: boolean;
  readonly sims: readonly SimChoice[];
  readonly onSend: (to: string, text: string, sim: number | null) => void;
  readonly onRetry: (clientId: number) => void;
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
