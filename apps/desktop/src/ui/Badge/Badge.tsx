import type { ReactNode } from 'react';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import './Badge.css';

/** Status tones. Amber = sensitive, red = destructive/failed, accent = act. */
export type Tone = 'neutral' | 'accent' | 'warning' | 'danger' | 'success';

export interface BadgeProps {
  readonly tone?: Tone;
  readonly icon?: IconName;
  readonly children: ReactNode;
}

/** A status label. Never a call to action. */
export function Badge({ tone = 'neutral', icon, children }: BadgeProps) {
  return (
    <span className={cx('p-badge', tone !== 'neutral' && `p-badge--${tone}`)}>
      {icon && <Icon name={icon} size="sm" />}
      {children}
    </span>
  );
}
