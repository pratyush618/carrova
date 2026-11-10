import type { ComponentPropsWithoutRef, ReactNode } from 'react';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import './Button.css';

export type ButtonKind = 'primary' | 'secondary' | 'quiet' | 'quiet-danger' | 'danger';

type NativeButton = Omit<ComponentPropsWithoutRef<'button'>, 'className' | 'type' | 'children'>;

export interface ButtonProps extends NativeButton {
  /** One `primary` per screen; its label says what happens. */
  readonly kind?: ButtonKind;
  readonly size?: 'md' | 'sm' | 'lg';
  /** Waiting on something: shows a spinner (pair with `disabled` and a "Waiting…" label). */
  readonly busy?: boolean;
  /** Leading icon. */
  readonly icon?: IconName;
  /** Trailing icon (mirrors in RTL when directional). */
  readonly iconEnd?: IconName;
  readonly children: ReactNode;
}

/** The one button. Labels are required; icon-only buttons use {@link IconButton}. */
export function Button({
  kind = 'secondary',
  size = 'md',
  busy = false,
  icon,
  iconEnd,
  children,
  ...native
}: ButtonProps) {
  return (
    <button
      type="button"
      className={cx(
        'p-btn',
        kind !== 'secondary' && `p-btn--${kind}`,
        size !== 'md' && `p-btn--${size}`,
      )}
      aria-busy={busy || undefined}
      {...native}
    >
      {icon && <Icon name={icon} />}
      {children}
      {iconEnd && <Icon name={iconEnd} />}
    </button>
  );
}

export interface IconButtonProps extends NativeButton {
  readonly icon: IconName;
  /** Accessible name; there is no visible text. */
  readonly label: string;
}

/** A quiet, icon-only button (close, dismiss). */
export function IconButton({ icon, label, ...native }: IconButtonProps) {
  return (
    <button type="button" className="p-btn p-btn--quiet p-btn--icon" aria-label={label} {...native}>
      <Icon name={icon} />
    </button>
  );
}

export interface LinkButtonProps extends NativeButton {
  readonly children: ReactNode;
}

/** An inline text action that looks like a link (it acts in-app, so it is a button). */
export function LinkButton({ children, ...native }: LinkButtonProps) {
  return (
    <button type="button" className="p-link" {...native}>
      {children}
    </button>
  );
}
