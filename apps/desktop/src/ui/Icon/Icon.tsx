import { cx } from '../cx';
import { DIRECTIONAL, ICONS, type IconName } from './icons';
import './Icon.css';

export type IconSize = 'sm' | 'md' | 'lg';

export interface IconProps {
  readonly name: IconName;
  readonly size?: IconSize;
  readonly className?: string;
}

/** Decorative line icon. Always `aria-hidden`: meaning comes from the text next to it. */
export function Icon({ name, size = 'md', className }: IconProps) {
  return (
    <svg
      className={cx(
        'p-icon',
        size !== 'md' && `p-icon--${size}`,
        DIRECTIONAL.has(name) && 'p-icon--dir',
        className,
      )}
      viewBox="0 0 24 24"
      aria-hidden="true"
      focusable="false"
    >
      {ICONS[name].map((d) => (
        <path key={d} d={d} />
      ))}
    </svg>
  );
}
