import { useEffect, useId, useRef, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import type { VisibilityChoice } from '../../engine/types';
import { useFormat } from '../../i18n/useFormat';
import { Icon } from '../../ui';
import { cx } from '../../ui/cx';
import { VISIBILITY_CHOICES, VISIBILITY_KEY, type VisibilityModel } from './model';

/**
 * The page's only word on visibility: who can find this computer, and as
 * what name. Opens the three choices and a way to Nearby settings; closing
 * hands the keyboard back to the pill (the Bell's pattern).
 */
export function VisibilityPill({
  visibility,
  me,
  onChoose,
  onSettings,
}: {
  readonly visibility: VisibilityModel;
  readonly me: string | null;
  readonly onChoose: (choice: VisibilityChoice) => void;
  readonly onSettings: () => void;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const pill = useRef<HTMLButtonElement>(null);
  const text = useRef<HTMLSpanElement>(null);
  const menuId = useId();

  // The pill cuts a long name short; the title carries it in full, as on the map.
  useEffect(() => {
    if (pill.current && text.current) pill.current.title = text.current.textContent;
  });

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (event: PointerEvent) => {
      if (!(event.target instanceof Node && root.current?.contains(event.target))) setOpen(false);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      setOpen(false);
      pill.current?.focus();
    };
    document.addEventListener('pointerdown', onPointerDown);
    document.addEventListener('keydown', onKeyDown);
    return () => {
      document.removeEventListener('pointerdown', onPointerDown);
      document.removeEventListener('keydown', onKeyDown);
    };
  }, [open]);

  /** An item of the menu: close it, give the pill the keyboard back, then run. */
  const act = (run: () => void) => () => {
    setOpen(false);
    pill.current?.focus();
    run();
  };
  const who = t(`nearby.visibility.who.${VISIBILITY_KEY[visibility.choice]}`);
  // Hidden from everyone, the name is shown to no one either.
  const named = me !== null && visibility.choice !== 'no_one';

  // The menu follows the pill in the DOM, so Tab reaches it; CSS shows it above.
  return (
    <div className="s-near__vis" ref={root}>
      <button
        ref={pill}
        type="button"
        className="s-near__pill"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        onClick={() => {
          setOpen((was) => !was);
        }}
      >
        <span
          className={cx('s-near__dot', visibility.choice === 'no_one' && 's-near__dot--off')}
          aria-hidden="true"
        />
        <span className="s-near__pill-text" ref={text}>
          <Trans
            i18nKey={named ? 'nearby.visibility.pill' : 'nearby.visibility.pillUnnamed'}
            values={{ who, name: me }}
            components={{ b: <b /> }}
          />
          {visibility.until !== null &&
            ` ${t('nearby.visibility.until', { time: format.time(visibility.until) })}`}
        </span>
        <Icon name="chev" />
      </button>
      {open && (
        <div
          id={menuId}
          className="s-near__vismenu"
          role="group"
          aria-label={t('nearby.visibility.label')}
        >
          {VISIBILITY_CHOICES.map((choice) => (
            <button
              key={choice}
              type="button"
              aria-pressed={choice === visibility.choice}
              onClick={act(() => {
                onChoose(choice);
              })}
            >
              <span className="s-near__vtick">
                {choice === visibility.choice && <Icon name="check" />}
              </span>
              <span>
                <b>{t(`nearby.visibility.choices.${VISIBILITY_KEY[choice]}.title`)}</b>
                <span>{t(`nearby.visibility.choices.${VISIBILITY_KEY[choice]}.body`)}</span>
              </span>
            </button>
          ))}
          <hr />
          <button type="button" onClick={act(onSettings)}>
            <span className="s-near__vtick">
              <Icon name="settings" />
            </span>
            <span>
              <b>{t('nearby.visibility.settings.title')}</b>
              <span>{t('nearby.visibility.settings.body')}</span>
            </span>
          </button>
        </div>
      )}
    </div>
  );
}
