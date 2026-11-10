import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import './ChoiceTile.css';

export interface ChoiceTileProps {
  readonly icon: IconName;
  readonly title: string;
  readonly sub: string;
  readonly onClick: () => void;
}

/** A big choice (Home's Send / Receive). */
export function ChoiceTile({ icon, title, sub, onClick }: ChoiceTileProps) {
  return (
    <button type="button" className="p-tile-choice" onClick={onClick}>
      <span className="p-tile-choice__icon">
        <Icon name={icon} size="lg" />
      </span>
      <span className="p-tile-choice__title">{title}</span>
      <span className="p-tile-choice__sub">{sub}</span>
    </button>
  );
}
