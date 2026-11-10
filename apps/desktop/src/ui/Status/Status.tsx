import type { ReactNode } from 'react';
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

/** The trust signal: a lock and a literal statement about encryption or signatures. */
export function LockLine({ children }: { readonly children: ReactNode }) {
  return (
    <span className="p-lockline">
      <Icon name="lock" size="sm" />
      {children}
    </span>
  );
}

/** A key or literal the user types. */
export function Kbd({ children }: { readonly children?: ReactNode }) {
  return <kbd className="p-kbd">{children}</kbd>;
}
