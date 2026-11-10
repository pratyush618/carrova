import type { ReactNode } from 'react';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import { Segmented, type SegmentOption } from '../Segmented/Segmented';
import './Decision.css';

export interface DecisionProps<T extends string> {
  readonly icon: IconName;
  /** The question, in plain words ("Your SSH keys"). */
  readonly title: string;
  /** The consequence, literally ("Anyone with a copy can log in as you"). */
  readonly why: ReactNode;
  readonly options: readonly SegmentOption<T>[];
  /** `null` until the user answers; approve stays disabled meanwhile. */
  readonly value: T | null;
  readonly onChange: (value: T) => void;
}

/** A sensitive or blocking question that needs an explicit answer. */
export function Decision<T extends string>({
  icon,
  title,
  why,
  options,
  value,
  onChange,
}: DecisionProps<T>) {
  return (
    <div className="p-decision">
      <p className="p-decision__title">
        <Icon name={icon} size="sm" />
        {title}
      </p>
      <p className="p-decision__why">{why}</p>
      <Segmented label={title} options={options} value={value} onChange={onChange} />
    </div>
  );
}
