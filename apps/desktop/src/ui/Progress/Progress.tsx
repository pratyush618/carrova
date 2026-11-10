import type { ReactNode } from 'react';
import { cx } from '../cx';
import './Progress.css';

export interface ProgressProps {
  /** 0–1. Clamped. */
  readonly value: number;
  readonly label: string;
  readonly paused?: boolean;
  /** Left and right captions under the bar (done, time left). */
  readonly meta?: readonly [ReactNode, ReactNode];
}

/** A real progress bar: never faked, never indeterminate. */
export function Progress({ value, label, paused = false, meta }: ProgressProps) {
  const p = Math.min(1, Math.max(0, value));
  return (
    <div>
      <div
        className={cx('p-progress', paused && 'p-progress--paused')}
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(p * 100)}
      >
        <div className="p-progress__fill" style={{ '--p': String(p) }} />
      </div>
      {meta && (
        <div className="p-progress-meta">
          <span>{meta[0]}</span>
          <span>{meta[1]}</span>
        </div>
      )}
    </div>
  );
}
