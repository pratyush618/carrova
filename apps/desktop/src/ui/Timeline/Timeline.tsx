import type { ReactNode } from 'react';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import './Timeline.css';

export interface TimelineEntry {
  readonly id: string;
  /** What happened ("Placed 312 files"). */
  readonly label: ReactNode;
  /** One line under it ("Checked against the sender's hashes"). */
  readonly detail?: ReactNode;
  /** Machine-readable time (ISO 8601), shown as `time`. */
  readonly dateTime?: string;
  readonly time?: string;
  /** `done` (default) = green tick, `accent` = still holding (undo point), `muted` = rewound. */
  readonly tone?: 'done' | 'accent' | 'muted';
  readonly icon?: IconName;
}

const DEFAULT_ICON: Record<NonNullable<TimelineEntry['tone']>, IconName> = {
  done: 'check',
  accent: 'clock',
  muted: 'undo',
};

export interface TimelineProps {
  readonly entries: readonly TimelineEntry[];
  /** The steps were just undone: they fold back in, newest first. */
  readonly rewind?: boolean;
}

/** A move, step by step: vertical, in order, one dot per step. */
export function Timeline({ entries, rewind = false }: TimelineProps) {
  return (
    <ol className={cx('p-timeline', rewind && 'p-timeline--rewind')}>
      {entries.map(({ id, label, detail, dateTime, time, tone = 'done', icon }) => (
        <li key={id}>
          <span className={`p-timeline__dot p-timeline__dot--${tone}`}>
            <Icon name={icon ?? DEFAULT_ICON[tone]} size="sm" />
          </span>
          <div>
            <b>{label}</b>
            {detail !== undefined && <span>{detail}</span>}
            {time !== undefined && <time dateTime={dateTime}>{time}</time>}
          </div>
        </li>
      ))}
    </ol>
  );
}

export interface MoveCardProps {
  /** Who and where ("From ezio-thinkpad" + OS badges). */
  readonly route: ReactNode;
  readonly when: ReactNode;
  /** Counts ("8 apps · 312 files · 38.6 MB"). */
  readonly sub: ReactNode;
  /** A status chip ("Arrived", "Undone"). */
  readonly tag?: ReactNode;
  /** This card's record is open beside the list. */
  readonly current?: boolean;
  /** Rolled back: shown dimmed. */
  readonly undone?: boolean;
  readonly onSelect: () => void;
}

/** One move on the History timeline; selecting it opens its record. */
export function MoveCard({
  route,
  when,
  sub,
  tag,
  current = false,
  undone = false,
  onSelect,
}: MoveCardProps) {
  return (
    <button
      type="button"
      className={cx('p-move', undone && 'p-move--undone')}
      aria-current={current || undefined}
      onClick={onSelect}
    >
      <span className="p-move__route">{route}</span>
      <span className="p-move__when">{when}</span>
      <span className="p-move__sub">{sub}</span>
      {tag !== undefined && <span className="p-move__tag">{tag}</span>}
    </button>
  );
}

/** Literal log lines (already redacted by the engine). Always left-to-right. */
export function Log({ lines }: { readonly lines: readonly string[] }) {
  return (
    <pre className="p-log" dir="ltr" tabIndex={0}>
      {lines.join('\n')}
    </pre>
  );
}
