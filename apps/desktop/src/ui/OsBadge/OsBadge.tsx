import type { ReactElement } from 'react';
import type { OsKind } from '../../bindings/OsKind';
import { cx } from '../cx';
import './OsBadge.css';

/** Neutral shapes, not vendor logos (16-unit grid). */
const GLYPH: Record<OsKind, ReactElement> = {
  linux: (
    <>
      <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="2" />
      <path d="M4.5 6.5 6.5 8l-2 1.5M8.5 10h3" />
    </>
  ),
  mac_os: (
    <>
      <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="2.5" />
      <path d="M1.75 6h12.5" />
      <circle className="p-os__dot" cx="4" cy="4.4" r=".6" />
      <circle className="p-os__dot" cx="5.9" cy="4.4" r=".6" />
    </>
  ),
  windows: (
    <>
      <rect x="2" y="2" width="5.25" height="5.25" rx="1" />
      <rect x="8.75" y="2" width="5.25" height="5.25" rx="1" />
      <rect x="2" y="8.75" width="5.25" height="5.25" rx="1" />
      <rect x="8.75" y="8.75" width="5.25" height="5.25" rx="1" />
    </>
  ),
};

export interface OsBadgeProps {
  readonly os: OsKind;
  /** What to call it here: an OS name ("Ubuntu 24.04") or a computer ("ezio-mbp"). */
  readonly label: string;
  readonly size?: 'md' | 'sm';
}

/** An operating system (or a computer on one), with a neutral glyph. */
export function OsBadge({ os, label, size = 'md' }: OsBadgeProps) {
  return (
    <span className={cx('p-os', size === 'sm' && 'p-os--sm')}>
      <svg viewBox="0 0 16 16" aria-hidden="true" focusable="false">
        {GLYPH[os]}
      </svg>
      {label}
    </span>
  );
}
