import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, IconButton } from '../Button/Button';
import { cx } from '../cx';
import { useMediaQuery } from '../motion';
import { Stepper } from '../Stepper/Stepper';
import { useChrome } from './chrome';
import { Sheet } from './Sheet';
import './layouts.css';

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

/** Below this width the rail folds into a header button and a side sheet. */
const RAIL_FITS = '(min-width: 960px)';

export interface FlowLayoutProps {
  readonly children: ReactNode;
  readonly actions: ReactNode;
  /** The move's manifest, beside the main column (a sheet on narrow windows). */
  readonly rail?: ReactNode;
  /** The route chip at the end of the header ("ezio-thinkpad → ezio-mbp"). */
  readonly route?: ReactNode;
  /** 0-based step to show as current when it differs from the route (pairing done, now "Their review"). */
  readonly stepNow?: number;
}

/** Tasks: close, labelled steps, the main column, the manifest rail and one action bar. */
export function FlowLayout({ children, actions, rail, route, stepNow }: FlowLayoutProps) {
  const { t } = useTranslation();
  const { flow } = useChrome();
  const railFits = useMediaQuery(RAIL_FITS);
  const [railOpen, setRailOpen] = useState(false);
  const sheetOpen = rail !== undefined && railOpen && !railFits;
  const closeRail = () => {
    setRailOpen(false);
  };
  return (
    <>
      <header className="p-flowhead" inert={sheetOpen}>
        <IconButton icon="close" label={t('flow.close')} onClick={flow?.onClose} />
        <span className="p-flowhead__name">{flow?.name}</span>
        {flow && <Stepper steps={flow.steps} current={stepNow ?? flow.step - 1} />}
        <span className="p-flowhead__end">
          {route}
          {rail !== undefined && !railFits && (
            <Button
              kind="quiet"
              size="sm"
              icon="box"
              aria-haspopup="dialog"
              onClick={() => {
                setRailOpen(true);
              }}
            >
              {t('flow.thisMove')}
            </Button>
          )}
        </span>
      </header>
      <div
        className={cx('p-flowbody', rail !== undefined && railFits && 'p-flowbody--rail')}
        inert={sheetOpen}
      >
        <main className="p-flow">
          <div className="p-flow__col">{children}</div>
        </main>
        {rail !== undefined && railFits && (
          <aside className="p-rail" aria-label={t('flow.thisMove')}>
            {rail}
          </aside>
        )}
      </div>
      <div className="p-flowfoot" inert={sheetOpen}>
        {actions}
      </div>
      {sheetOpen && (
        <Sheet title={t('flow.thisMove')} onClose={closeRail}>
          {rail}
        </Sheet>
      )}
    </>
  );
}

/** Centred content inside a flow column (codes, stamps). */
export function FlowCenter({ children }: { readonly children: ReactNode }) {
  return <div className="p-flow__center">{children}</div>;
}
