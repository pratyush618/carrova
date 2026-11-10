import { useId, type ComponentPropsWithoutRef, type ReactNode } from 'react';
import { cx } from '../cx';
import './Field.css';

/** Ids a field hands to its control so label, hint and error are announced. */
export interface FieldControl {
  readonly id: string;
  readonly 'aria-describedby'?: string;
  readonly 'aria-invalid'?: true;
}

export interface FieldProps {
  readonly label: ReactNode;
  readonly hint?: ReactNode;
  /** Replaces the hint and marks the control invalid. */
  readonly error?: ReactNode;
  /** Extra content between the control and the hint (a strength meter). */
  readonly below?: ReactNode;
  readonly children: (control: FieldControl) => ReactNode;
}

/** Label + control + hint/error, wired for assistive tech. */
export function Field({ label, hint, error, below, children }: FieldProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  const message = error ?? hint;
  const control: FieldControl = {
    id,
    ...(message !== undefined && { 'aria-describedby': hintId }),
    ...(error !== undefined && { 'aria-invalid': true }),
  };
  return (
    <div className="p-field">
      <label className="p-field__label" htmlFor={id}>
        {label}
      </label>
      {children(control)}
      {below}
      {message !== undefined && (
        <p
          id={hintId}
          className={cx('p-field__hint', error !== undefined && 'p-field__hint--error')}
        >
          {message}
        </p>
      )}
    </div>
  );
}

export interface InputProps extends Omit<ComponentPropsWithoutRef<'input'>, 'className'> {
  readonly mono?: boolean;
}

/** A text input. Monospace inputs (addresses, paths) are forced left-to-right. */
export function Input({ mono = false, ...native }: InputProps) {
  return (
    <input
      className={cx('p-input', mono && 'p-input--mono')}
      dir={mono ? 'ltr' : undefined}
      {...native}
    />
  );
}
