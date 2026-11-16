import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { ThreadRow } from '../../../bindings/ThreadRow';
import type { SmsSend } from '../../../flows/phone/useSmsSends';
import { Input } from '../../../ui';
import { Bubbles } from './Bubbles';
import { Composer } from './Composer';
import { initials } from './ThreadList';

/**
 * A new text: a number typed in To, or a recent conversation picked from
 * the suggestions (names come from the threads; there is no address book).
 */
export function NewMessage({
  threads,
  sends,
  phone,
  away,
  onSelect,
  onSend,
  onRetry,
}: {
  readonly threads: readonly ThreadRow[];
  readonly sends: readonly SmsSend[];
  readonly phone: string;
  readonly away: boolean;
  readonly onSelect: (id: number) => void;
  readonly onSend: (to: string, text: string) => void;
  readonly onRetry: (clientId: number) => void;
}) {
  const { t } = useTranslation();
  const [to, setTo] = useState('');
  const query = to.trim().toLowerCase();
  const suggestions =
    query === ''
      ? []
      : threads.filter(
          (row) => row.name?.toLowerCase().includes(query) === true || row.address.includes(query),
        );
  return (
    <section className="s-thread" aria-labelledby="s-thread-title">
      <header className="s-thread__head">
        <h2 id="s-thread-title">{t('ecosystem.messages.newMessage')}</h2>
      </header>
      <div className="s-thread__to">
        <label htmlFor="s-thread-to">{t('ecosystem.messages.to')}</label>
        <Input
          id="s-thread-to"
          placeholder={t('ecosystem.messages.toHint')}
          value={to}
          onChange={(e) => {
            setTo(e.target.value);
          }}
        />
      </div>
      {suggestions.length > 0 ? (
        <ul className="s-thread__sugg">
          {suggestions.map((row) => (
            <li key={row.id}>
              <button
                type="button"
                className="s-conv"
                onClick={() => {
                  onSelect(row.id);
                }}
              >
                <span className="s-av" aria-hidden="true">
                  {initials(row.name)}
                </span>
                <span className="s-conv__t">
                  <b>{row.name ?? row.address}</b>
                  <span>{row.address}</span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <Bubbles messages={[]} sends={sends} phone={phone} onRetry={onRetry} />
      )}
      <Composer
        phone={phone}
        away={away}
        disabled={query === ''}
        onSend={(text) => {
          onSend(to.trim(), text);
        }}
      />
    </section>
  );
}
