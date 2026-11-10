import { useRef, type KeyboardEvent } from 'react';
import { cx } from '../cx';
import './Segmented.css';

export interface SegmentOption<T extends string> {
  readonly value: T;
  readonly label: string;
}

export interface SegmentedProps<T extends string> {
  /** Accessible name of the group. */
  readonly label: string;
  readonly options: readonly SegmentOption<T>[];
  /** `null` = not answered yet (nothing preselected). */
  readonly value: T | null;
  readonly onChange: (value: T) => void;
  /** The answer is a risk (bring keys): the picked option reads amber. */
  readonly warn?: boolean;
}

const STEP: Partial<Record<string, number>> = {
  ArrowRight: 1,
  ArrowDown: 1,
  ArrowLeft: -1,
  ArrowUp: -1,
};

/**
 * An explicit choice between a few answers, as a radio group: one tab stop,
 * arrow keys move the answer (mirrored in RTL), nothing preselected unless
 * the caller passes a value.
 */
export function Segmented<T extends string>({
  label,
  options,
  value,
  onChange,
  warn = false,
}: SegmentedProps<T>) {
  const group = useRef<HTMLDivElement>(null);
  const current = options.findIndex((o) => o.value === value);
  const tabStop = current === -1 ? 0 : current;

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = STEP[event.key];
    if (step === undefined) return;
    event.preventDefault();
    const rtl = group.current?.closest('[dir]')?.getAttribute('dir') === 'rtl';
    const horizontal = event.key === 'ArrowLeft' || event.key === 'ArrowRight';
    const delta = rtl && horizontal ? -step : step;
    const next = (tabStop + delta + options.length) % options.length;
    const option = options[next];
    if (!option) return;
    onChange(option.value);
    group.current?.querySelectorAll<HTMLButtonElement>('[role=radio]')[next]?.focus();
  };

  return (
    <div
      ref={group}
      className={cx('p-seg', warn && 'p-seg--warn')}
      role="radiogroup"
      aria-label={label}
      onKeyDown={onKeyDown}
    >
      {options.map((option, i) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          className="p-seg__opt"
          aria-checked={option.value === value}
          tabIndex={i === tabStop ? 0 : -1}
          onClick={() => {
            onChange(option.value);
          }}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
