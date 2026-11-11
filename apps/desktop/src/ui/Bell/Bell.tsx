import { useEffect, useId, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { AppGlyph } from '../AppGlyph/AppGlyph';
import { Button, LinkButton } from '../Button/Button';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import './Bell.css';

/** One notice in the bell's list. */
export interface BellRow {
  readonly id: number;
  readonly icon: IconName;
  readonly tone: 'neutral' | 'accent' | 'warning' | 'success';
  /** The other computer, shown in bold before `summary`. */
  readonly peer: string | null;
  readonly summary: string;
  /** "14:02", "Yesterday, 14:02", "9 Nov". */
  readonly when: string;
  readonly unread: boolean;
  /** A request: offers Review. */
  readonly review: boolean;
}

/** Notices under one heading ("Today", "Earlier"). */
export interface BellGroup {
  readonly title: string;
  readonly rows: readonly BellRow[];
}

/** What the bell shows and does; the shell supplies it. */
export interface BellModel {
  readonly unread: number;
  readonly groups: readonly BellGroup[];
  readonly onReview: (id: number) => void;
  readonly onMarkAllRead: () => void;
  readonly onSettings: () => void;
}

/**
 * The bell: a header button with the count of new notices, opening a list
 * over the current page. It never navigates by itself; it closes on a second
 * click, an outside click, Escape, or when one of its own actions runs
 * (leaving the page remounts it closed).
 */
export function Bell({ model }: { readonly model: BellModel }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const listId = useId();

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!(event.target instanceof Node && root.current?.contains(event.target))) setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      setOpen(false);
      button.current?.focus();
    };
    document.addEventListener('pointerdown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [open]);

  /** An action of the list: run it, then close the list. */
  const act = (run: () => void) => () => {
    setOpen(false);
    run();
  };

  return (
    <div className="p-bell" ref={root}>
      <button
        ref={button}
        type="button"
        className="p-bell__btn"
        aria-label={
          model.unread > 0 ? t('bell.labelUnread', { count: model.unread }) : t('bell.label')
        }
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        onClick={() => {
          setOpen((was) => !was);
        }}
      >
        <Icon name="bell" />
        {model.unread > 0 && (
          <span className="p-bell__dot" aria-hidden="true">
            {model.unread}
          </span>
        )}
      </button>
      {open && (
        <section id={listId} className="p-bell__pop" aria-label={t('bell.label')}>
          <header className="p-bell__head">
            <b>{t('bell.label')}</b>
            {model.unread > 0 && (
              <LinkButton onClick={model.onMarkAllRead}>{t('bell.markAllRead')}</LinkButton>
            )}
          </header>
          {model.groups.length === 0 ? (
            <p className="p-bell__empty">{t('bell.empty')}</p>
          ) : (
            model.groups.map((group) => (
              <div key={group.title}>
                <h2 className="p-bell__day">{group.title}</h2>
                <ul className="p-bell__list">
                  {group.rows.map((row) => (
                    <li key={row.id} className="p-bell__note" data-unread={row.unread || undefined}>
                      <AppGlyph glyph={{ icon: row.icon }} tone={row.tone} />
                      <div className="p-bell__body">
                        <p>
                          {row.peer !== null && <b>{row.peer} </b>}
                          {row.summary}
                        </p>
                        <p className="p-bell__when">{row.when}</p>
                        {row.review && (
                          <Button
                            kind="primary"
                            size="sm"
                            onClick={act(() => {
                              model.onReview(row.id);
                            })}
                          >
                            {t('bell.review')}
                          </Button>
                        )}
                      </div>
                    </li>
                  ))}
                </ul>
              </div>
            ))
          )}
          <footer className="p-bell__foot">
            <span>{t('bell.kept')}</span>
            <LinkButton onClick={act(model.onSettings)}>{t('bell.settings')}</LinkButton>
          </footer>
        </section>
      )}
    </div>
  );
}
