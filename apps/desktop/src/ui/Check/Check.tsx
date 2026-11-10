import { Icon } from '../Icon/Icon';
import './Check.css';

/** `mixed` = some children picked (tree parents). */
export type CheckValue = boolean | 'mixed';

export interface CheckProps {
  readonly value: CheckValue;
  readonly label: string;
  readonly onChange: (picked: boolean) => void;
}

/** A checkbox. Clicking a mixed box picks everything under it. */
export function Check({ value, label, onChange }: CheckProps) {
  return (
    <button
      type="button"
      role="checkbox"
      className="p-check"
      aria-checked={value}
      aria-label={label}
      onClick={() => {
        onChange(value !== true);
      }}
    >
      <Icon name="check" />
    </button>
  );
}

/** Placeholder box for items that cannot be picked (unmovable); not focusable. */
export function LockedCheck() {
  return <span className="p-check p-check--locked" aria-hidden="true" />;
}
