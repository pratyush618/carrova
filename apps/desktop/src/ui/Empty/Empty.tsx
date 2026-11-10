import type { ReactNode } from 'react';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import './Empty.css';

export interface EmptyProps {
  readonly art: IconName;
  readonly tone?: 'neutral' | 'danger';
  readonly title: ReactNode;
  /** What happened and why. */
  readonly body: ReactNode;
  /** The one recovery action. */
  readonly action?: ReactNode;
}

/**
 * Empty and error states share one shape: what happened, why, and one
 * recovery action. The title is the screen's heading.
 */
export function Empty({ art, tone = 'neutral', title, body, action }: EmptyProps) {
  return (
    <div className="p-empty">
      <div className={cx('p-empty__art', tone === 'danger' && 'p-empty__art--danger')}>
        <Icon name={art} size="lg" />
      </div>
      <h1 className="p-empty__title" tabIndex={-1} data-autofocus>
        {title}
      </h1>
      <p>{body}</p>
      {action !== undefined && <div className="p-empty__action">{action}</div>}
    </div>
  );
}
