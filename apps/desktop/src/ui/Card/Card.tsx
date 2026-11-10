import type { ReactNode } from 'react';
import { cx } from '../cx';
import './Card.css';

export interface CardProps {
  /** Optional heading row: title on the start side, `aside` on the end. */
  readonly title?: ReactNode;
  readonly aside?: ReactNode;
  readonly pad?: boolean;
  readonly children: ReactNode;
}

/** A grouping surface. */
export function Card({ title, aside, pad = true, children }: CardProps) {
  return (
    <section className={cx('p-card', pad && 'p-card--pad')}>
      {title !== undefined && (
        <div className="p-card__head">
          <h3>{title}</h3>
          {aside}
        </div>
      )}
      {children}
    </section>
  );
}
