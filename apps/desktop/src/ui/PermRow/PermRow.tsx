import type { ReactNode } from 'react';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import './PermRow.css';

export interface PermRowProps {
  readonly icon: IconName;
  /** The permission in plain words ("Read one folder"). */
  readonly title: string;
  /** Exactly what it covers. */
  readonly detail: ReactNode;
  /** A chip on the inline end ("Read only"). */
  readonly tag?: ReactNode;
  /** A restriction that protects the user ("No internet"): green tile. */
  readonly safe?: boolean;
}

/** One thing a plugin may (or may not) do. */
export function PermRow({ icon, title, detail, tag, safe = false }: PermRowProps) {
  return (
    <li className="p-perm">
      <span className={cx('p-perm__ic', safe && 'p-perm__ic--safe')}>
        <Icon name={icon} />
      </span>
      <span className="p-perm__t">{title}</span>
      {tag !== undefined && <span className="p-perm__tag">{tag}</span>}
      <span className="p-perm__d">{detail}</span>
    </li>
  );
}
