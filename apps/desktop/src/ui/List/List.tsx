import type { ReactNode } from 'react';
import { cx } from '../cx';
import './List.css';

export interface ListProps {
  /** `card` sits on its own surface; `plain` is dividers on the canvas. */
  readonly kind?: 'card' | 'plain';
  readonly label?: string;
  readonly children: ReactNode;
}

/** A list of {@link Row}s. */
export function List({ kind = 'card', label, children }: ListProps) {
  return (
    <ul className={cx('p-list', `p-list--${kind}`)} aria-label={label}>
      {children}
    </ul>
  );
}

export interface RowContent {
  readonly lead?: ReactNode;
  readonly title: ReactNode;
  readonly sub?: ReactNode;
  readonly trail?: ReactNode;
  /** Dimmed: shown but not moving. */
  readonly off?: boolean;
}

function RowBody({ lead, title, sub, trail }: RowContent) {
  return (
    <>
      <span className="p-row__lead">{lead}</span>
      <span>
        <span className="p-row__title">{title}</span>
        {sub !== undefined && <span className="p-row__sub">{sub}</span>}
      </span>
      <span className="p-row__trail">{trail}</span>
    </>
  );
}

/** One list row: lead | title + sub | trail. Must sit inside a {@link List}. */
export function Row({ off = false, ...content }: RowContent) {
  return (
    <li className={cx('p-row', off && 'p-row--off')}>
      <RowBody {...content} />
    </li>
  );
}

export interface LinkRowProps extends RowContent {
  readonly onClick: () => void;
}

/** A row that drills down (opens a sheet or page). The whole row is the button. */
export function LinkRow({ onClick, off = false, ...content }: LinkRowProps) {
  return (
    <li>
      <button
        type="button"
        className={cx('p-row p-row--link', off && 'p-row--off')}
        onClick={onClick}
      >
        <RowBody {...content} />
      </button>
    </li>
  );
}

/** A row outside any list (a single setting on the canvas). */
export function StandaloneRow(content: RowContent) {
  return (
    <div className="p-row p-row--flush">
      <RowBody {...content} />
    </div>
  );
}
