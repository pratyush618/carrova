import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import './Check.css';

/** `mixed` = some children picked (tree parents). */
export type CheckValue = boolean | 'mixed';

export interface CheckProps {
  readonly value: CheckValue;
  readonly label: string;
  readonly onChange: (picked: boolean) => void;
  /** A sensitive item (keys, tokens): amber instead of the accent. */
  readonly warn?: boolean;
}

/** A tri-state checkbox. Clicking a mixed box picks everything under it. */
export function Check({ value, label, onChange, warn = false }: CheckProps) {
  return (
    <button
      type="button"
      role="checkbox"
      className={cx('p-check', warn && 'p-check--warn')}
      aria-checked={value}
      aria-label={label}
      onClick={() => {
        onChange(value !== true);
      }}
    >
      {value === 'mixed' ? <Icon name="minus" /> : value && <Icon name="check" />}
    </button>
  );
}

/** Placeholder box for items that cannot be picked (unmovable); not focusable. */
export function LockedCheck() {
  return <span className="p-check p-check--locked" aria-hidden="true" />;
}
