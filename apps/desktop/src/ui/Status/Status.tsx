import type { ReactNode } from 'react';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import './Status.css';

/** A live "waiting for…" line with a pulsing dot (static under reduced motion). */
export function Status({ children }: { readonly children: ReactNode }) {
  return (
    <p className="p-status" role="status">
      <span className="p-pulse" aria-hidden="true" />
      {children}
    </p>
  );
}

/**
 * The trust signal: a lock and a literal statement about encryption or signatures.
 * `quiet` drops it to secondary text where a banner already holds the eye.
 */
export function LockLine({
  quiet = false,
  children,
}: {
  readonly quiet?: boolean;
  readonly children: ReactNode;
}) {
  return (
    <span className={cx('p-lockline', quiet && 'p-lockline--quiet')}>
      <Icon name="lock" size="sm" />
      {children}
    </span>
  );
}

/** A key or literal the user types. */
export function Kbd({ children }: { readonly children?: ReactNode }) {
  return <kbd className="p-kbd">{children}</kbd>;
}
