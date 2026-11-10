import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { IconButton } from '../Button/Button';
import { cx } from '../cx';
import { useChrome } from './chrome';
import './layouts.css';

function StepDots({ step, total }: { readonly step: number; readonly total: number }) {
  const { t } = useTranslation();
  return (
    <span className="p-dots" role="img" aria-label={t('flow.step', { step, total })}>
      {Array.from({ length: total }, (_, i) => (
        <i
          key={i}
          className={cx(i + 1 < step && 'p-dots--done', i + 1 === step && 'p-dots--now')}
        />
      ))}
    </span>
  );
}

export interface ActionBarProps {
  /** Secondary actions at the start (Back, Cancel, Customize). */
  readonly start?: ReactNode;
  /** A live note before the end actions ("1.2 GB, no keys"). */
  readonly note?: ReactNode;
  /** The end actions; the primary button goes last. */
  readonly end?: ReactNode;
}

/** The flow's one action bar. */
export function ActionBar({ start, note, end }: ActionBarProps) {
  return (
    <footer className="p-actions">
      {start}
      {note !== undefined && (
        <span className="p-actions__note" aria-live="polite">
          {note}
        </span>
      )}
      <span className="p-actions__end">{end}</span>
    </footer>
  );
}

export interface FlowLayoutProps {
  readonly children: ReactNode;
  readonly actions: ReactNode;
}

/** Tasks: close, step dots, a centred column and one action bar. */
export function FlowLayout({ children, actions }: FlowLayoutProps) {
  const { t } = useTranslation();
  const { flow } = useChrome();
  return (
    <>
      <header className="p-flowbar">
        <span className="p-flowbar__close">
          <IconButton icon="close" label={t('flow.close')} onClick={flow?.onClose} />
        </span>
        <span className="p-flowbar__name">{flow?.name}</span>
        {flow && <StepDots step={flow.step} total={flow.total} />}
      </header>
      <main className="p-flow">
        <div className="p-flow__col">{children}</div>
      </main>
      {actions}
    </>
  );
}

/** Centred content inside a flow column (codes, stamps). */
export function FlowCenter({ children }: { readonly children: ReactNode }) {
  return <div className="p-flow__center">{children}</div>;
}

/** First run: nothing but the message. */
export function BareLayout({ children }: { readonly children: ReactNode }) {
  return (
    <main className="p-bare">
      <div className="p-flow__col">{children}</div>
    </main>
  );
}
