import type { ReactNode } from 'react';
import { cx } from '../cx';
import './Text.css';

export interface HeadProps {
  readonly title: ReactNode;
  readonly lede?: ReactNode;
  readonly hero?: boolean;
  readonly center?: boolean;
}

/**
 * The screen heading. `tabIndex=-1` lets the shell move focus here on every
 * screen change, so screen readers announce where the user is.
 */
export function Head({ title, lede, hero = false, center = false }: HeadProps) {
  return (
    <div className={cx('p-head', center && 'p-head--center')}>
      <h1 className={cx('p-title', hero && 'p-title--hero')} tabIndex={-1} data-autofocus>
        {title}
      </h1>
      {lede !== undefined && <p className="p-lede">{lede}</p>}
    </div>
  );
}

export interface GroupHeadProps {
  readonly title: ReactNode;
  /** Status on the inline end (a badge or a count). */
  readonly aside?: ReactNode;
}

/** A section heading inside a screen. */
export function GroupHead({ title, aside }: GroupHeadProps) {
  return (
    <div className="p-group-head">
      <h2>{title}</h2>
      {aside}
    </div>
  );
}

/** Small muted text. */
export function Note({
  children,
  center = false,
}: {
  readonly children: ReactNode;
  readonly center?: boolean;
}) {
  return <p className={cx('p-note', center && 'p-note--center')}>{children}</p>;
}

/** Muted body text. */
export function Muted({ children }: { readonly children: ReactNode }) {
  return <p className="p-muted">{children}</p>;
}

/** Vertical stack with the standard 12px gap. */
export function Stack({ children }: { readonly children: ReactNode }) {
  return <div className="p-stack">{children}</div>;
}

/** A path or code: monospace and always left-to-right. */
export function Path({ children }: { readonly children: ReactNode }) {
  return (
    <span className="p-mono p-ltr" dir="ltr">
      {children}
    </span>
  );
}
