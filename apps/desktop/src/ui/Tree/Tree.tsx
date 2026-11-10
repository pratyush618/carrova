import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import './Tree.css';

/** A tree of selectable items (Customize). */
export function Tree({
  label,
  children,
}: {
  readonly label: string;
  readonly children: ReactNode;
}) {
  return (
    <div className="p-tree" role="tree" aria-label={label}>
      {children}
    </div>
  );
}

export interface TreeRowProps {
  /** 0 = app, 1 = item, 2 = path. */
  readonly depth: number;
  /** Parents only: whether children are shown. */
  readonly expanded?: boolean;
  readonly onToggleExpanded?: () => void;
  /** The checkbox (or a locked placeholder). */
  readonly check: ReactNode;
  readonly name: ReactNode;
  /** Lines under the name: a path, a count, a reason. */
  readonly details?: ReactNode;
  /** Badges and size on the inline end. */
  readonly meta?: ReactNode;
  /** Shown but not movable. */
  readonly off?: boolean;
}

/** One row of a {@link Tree}. */
export function TreeRow({
  depth,
  expanded,
  onToggleExpanded,
  check,
  name,
  details,
  meta,
  off = false,
}: TreeRowProps) {
  const { t } = useTranslation();
  const parent = expanded !== undefined;
  return (
    <div
      className={cx('p-tree__row', off && 'p-tree__row--off')}
      role="treeitem"
      aria-level={depth + 1}
      aria-expanded={expanded}
      aria-selected={false}
      style={{ '--depth': String(depth) }}
    >
      {parent ? (
        <button
          type="button"
          className="p-tree__exp"
          aria-expanded={expanded}
          aria-label={expanded ? t('ui.tree.collapse') : t('ui.tree.expand')}
          onClick={onToggleExpanded}
        >
          <Icon name="chev" size="sm" />
        </button>
      ) : (
        <span />
      )}
      {check}
      <span className="p-tree__name">
        {depth === 0 ? <b>{name}</b> : name}
        {details}
      </span>
      <span className="p-tree__meta">{meta}</span>
    </div>
  );
}
