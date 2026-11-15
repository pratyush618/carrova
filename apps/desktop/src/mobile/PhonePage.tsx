import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { IconButton } from '../ui';
import { cx } from '../ui/cx';
import './mobile.css';

/**
 * One phone screen: a top bar (Back, the title, its actions) over a
 * scrolling column. Without a title the content brings its own heading
 * (the centred status screens).
 */
export function PhonePage({
  title,
  onBack,
  actions,
  center = false,
  children,
}: {
  readonly title?: string;
  readonly onBack?: () => void;
  readonly actions?: ReactNode;
  readonly center?: boolean;
  readonly children: ReactNode;
}) {
  const { t } = useTranslation();
  return (
    <main className="m-page">
      {(title !== undefined || onBack !== undefined) && (
        <header className="m-bar">
          {onBack !== undefined && (
            <IconButton icon="back" label={t('mobile.back')} onClick={onBack} />
          )}
          {title !== undefined && (
            <h1 tabIndex={-1} data-autofocus>
              {title}
            </h1>
          )}
          {actions}
        </header>
      )}
      <div className={cx('m-body', center && 'm-body--center')}>{children}</div>
    </main>
  );
}
