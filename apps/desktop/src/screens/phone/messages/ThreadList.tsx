import { useTranslation } from 'react-i18next';
import type { ThreadRow } from '../../../bindings/ThreadRow';
import { useFormat } from '../../../i18n/useFormat';
import { LinkButton } from '../../../ui';
import { cx } from '../../../ui/cx';

/** Two initials of a contact's name, or `#` for a bare number. */
export const initials = (name: string | null): string =>
  name === null
    ? '#'
    : name
        .split(/\s+/)
        .slice(0, 2)
        .map((w) => w.charAt(0).toUpperCase())
        .join('');

/** The conversations, newest first; the open one marked; then a way to older ones, if any. */
export function ThreadList({
  threads,
  selected,
  onSelect,
  onMore,
}: {
  readonly threads: readonly ThreadRow[];
  readonly selected: number | null;
  readonly onSelect: (id: number) => void;
  /** Given while older conversations wait on the phone. */
  readonly onMore?: () => void;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  // The engine counts the days; `0` shows the time, `1` Yesterday, more the date.
  const when = (row: ThreadRow) =>
    row.days_ago === 0
      ? format.time(new Date(row.when_ms))
      : row.days_ago === 1
        ? t('ecosystem.messages.yesterday')
        : format.date(new Date(row.when_ms));
  return (
    <nav className="s-msgs__list" aria-label={t('ecosystem.messages.list')}>
      <ul>
        {threads.map((row) => (
          <li key={row.id}>
            <button
              type="button"
              className={cx('s-conv', row.unread > 0 && 's-conv--unread')}
              aria-current={row.id === selected ? 'true' : undefined}
              onClick={() => {
                onSelect(row.id);
              }}
            >
              <span className="s-av" aria-hidden="true">
                {initials(row.name)}
              </span>
              <span className="s-conv__t">
                <b>
                  {row.name ?? row.address}
                  <small>{when(row)}</small>
                </b>
                <span>{row.snippet}</span>
                {row.unread > 0 && (
                  <span className="p-vh">
                    {t('ecosystem.messages.unread', { count: row.unread })}
                  </span>
                )}
              </span>
            </button>
          </li>
        ))}
      </ul>
      {onMore && (
        <p className="s-msgs__more">
          <LinkButton onClick={onMore}>{t('ecosystem.messages.olderThreads')}</LinkButton>
        </p>
      )}
    </nav>
  );
}
