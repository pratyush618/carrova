import { useId, useRef, type ReactNode } from 'react';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import { useFocusTrap } from './useFocusTrap';
import './overlays.css';

export type DialogTone = 'accent' | 'warning' | 'danger';

export interface DialogProps {
  readonly tone?: DialogTone;
  readonly icon: IconName;
  readonly title: string;
  /** Esc and the scrim take the safe way out. */
  readonly onDismiss: () => void;
  readonly children: ReactNode;
  /** Actions; mark the safe one `data-initial-focus` to focus it first. */
  readonly foot: ReactNode;
}

/** A risk or a confirmation. Modal: focus stays inside until it closes. */
export function Dialog({ tone = 'accent', icon, title, onDismiss, children, foot }: DialogProps) {
  const ref = useRef<HTMLElement>(null);
  const titleId = useId();
  useFocusTrap(ref, onDismiss);
  return (
    <>
      <div className="p-scrim" aria-hidden="true" onClick={onDismiss} />
      <section
        ref={ref}
        className={`p-dialog p-dialog--${tone}`}
        role={tone === 'accent' ? 'dialog' : 'alertdialog'}
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <span className="p-dialog__icon">
          <Icon name={icon} />
        </span>
        <h2 id={titleId} tabIndex={-1}>
          {title}
        </h2>
        <div className="p-dialog__body">{children}</div>
        <div className="p-dialog__foot">{foot}</div>
      </section>
    </>
  );
}
