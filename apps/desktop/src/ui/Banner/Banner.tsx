import type { ReactNode } from 'react';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import type { Tone } from '../Badge/Badge';
import './Banner.css';

export type BannerTone = Exclude<Tone, 'neutral'>;

const ICON: Record<BannerTone, IconName> = {
  accent: 'info',
  warning: 'key',
  danger: 'warn',
  success: 'check',
};

export interface BannerProps {
  readonly tone: BannerTone;
  readonly title: ReactNode;
  readonly body?: ReactNode;
  /** At most one action. */
  readonly action?: ReactNode;
  readonly icon?: IconName;
}

/** One message, at most one action. Danger banners interrupt (`alert`); the rest are polite. */
export function Banner({ tone, title, body, action, icon }: BannerProps) {
  return (
    <div className={`p-banner p-banner--${tone}`} role={tone === 'danger' ? 'alert' : 'status'}>
      <Icon name={icon ?? ICON[tone]} />
      <div>
        <p className="p-banner__title">{title}</p>
        {body !== undefined && <p className="p-banner__body">{body}</p>}
      </div>
      <div>{action}</div>
    </div>
  );
}
