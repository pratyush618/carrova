import { useId, useRef, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { IconButton } from '../Button/Button';
import { useFocusTrap } from './useFocusTrap';
import './layouts.css';
import './overlays.css';

export interface SheetProps {
  readonly title: string;
  /** Esc, the scrim and the close button all call this. */
  readonly onClose: () => void;
  readonly children: ReactNode;
  /** Footer: an optional note, then actions (primary last). */
  readonly foot: ReactNode;
}

/** Drill-down detail, sliding in from the inline end. Modal: focus stays inside. */
export function Sheet({ title, onClose, children, foot }: SheetProps) {
  const { t } = useTranslation();
  const ref = useRef<HTMLElement>(null);
  const titleId = useId();
  useFocusTrap(ref, onClose);
  return (
    <>
      <div className="p-scrim" aria-hidden="true" onClick={onClose} />
      <section
        ref={ref}
        className="p-sheet"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
      >
        <header className="p-sheet__head">
          <h2 id={titleId} tabIndex={-1}>
            {title}
          </h2>
          <IconButton icon="close" label={t('ui.close')} onClick={onClose} />
        </header>
        <div className="p-sheet__body">{children}</div>
        <footer className="p-sheet__foot">{foot}</footer>
      </section>
    </>
  );
}

/** A note at the start of a sheet footer ("398 of 418 files"). */
export function SheetNote({ children }: { readonly children: ReactNode }) {
  return (
    <span className="p-actions__note" aria-live="polite">
      {children}
    </span>
  );
}
