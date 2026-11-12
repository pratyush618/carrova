import { useEffect, useId, useRef, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import type { VisibilityChoice } from '../../engine/types';
import { useFormat } from '../../i18n/useFormat';
import { Icon } from '../../ui';
import { cx } from '../../ui/cx';
import { VISIBILITY_CHOICES, VISIBILITY_KEY, type VisibilityModel } from './model';

/**
 * The page's only word on visibility: who can find this computer, and as
 * what name. Opens the three choices and a way to Nearby settings.
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
  const menuId = useId();
  useEffect(() => {
    if (!open) return;
    const close = (event: Event) => {
      if (
        event instanceof KeyboardEvent
          ? event.key === 'Escape'
          : !root.current?.contains(event.target as Node)
      )
        setOpen(false);
    };
    document.addEventListener('keydown', close);
    document.addEventListener('pointerdown', close);
    return () => {
      document.removeEventListener('keydown', close);
      document.removeEventListener('pointerdown', close);
    };
  }, [open]);
  const who = t(`nearby.visibility.who.${VISIBILITY_KEY[visibility.choice]}`);
  const choose = (choice: VisibilityChoice) => {
    setOpen(false);
    onChoose(choice);
  };
  return (
    <div className="s-near__vis" ref={root}>
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
              onClick={() => {
                choose(choice);
              }}
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
          <button type="button" onClick={onSettings}>
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
      <button
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
        <span>
          <Trans
            i18nKey={me === null ? 'nearby.visibility.pillUnnamed' : 'nearby.visibility.pill'}
            values={{ who, name: me }}
            components={{ b: <b /> }}
          />
          {visibility.until !== null &&
            ` ${t('nearby.visibility.until', { time: format.time(visibility.until) })}`}
        </span>
        <Icon name="chev" />
      </button>
    </div>
  );
}
