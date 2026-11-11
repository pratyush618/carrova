import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import { useChrome } from './chrome';
import './layouts.css';

/** The Carrova wordmark. */
export function Wordmark() {
  const { t } = useTranslation();
  return (
    <span className="p-wordmark">
      <span className="p-wordmark__mark">
        <Icon name="swap" />
      </span>
      {t('brand.name')}
    </span>
  );
}

export interface HubLayoutProps {
  /**
   * `page` = a reading column; `center` = content centred in the page (Home);
   * `flush` = the screen lays out the whole page itself (History's two panes).
   */
  readonly variant?: 'page' | 'center' | 'flush';
  /** Page controls at the start of the header bar (a filter, a view switch). */
  readonly tools?: ReactNode;
  readonly children: ReactNode;
}

/** The slim bar above every hub page: its tools, then the bell at the end. */
export function HeaderBar({
  tools,
  bell,
}: {
  readonly tools?: ReactNode;
  readonly bell: ReactNode;
}) {
  return (
    <header className="p-hubhead">
      <div className="p-hubhead__tools">{tools}</div>
      {bell}
    </header>
  );
}

/** Places: the sidebar (main navigation, bundle state at its foot), then the page. */
export function HubLayout({ variant = 'page', tools, children }: HubLayoutProps) {
  const { t } = useTranslation();
  const { hubLinks, hubStatus } = useChrome();
  return (
    <div className="p-hub">
      <nav className="p-side" aria-label={t('nav.label')}>
        {hubLinks.map((link) => (
          <button
            key={link.id}
            type="button"
            className="p-side__link"
            aria-current={link.current ? 'page' : undefined}
            onClick={link.onSelect}
          >
            <Icon name={link.icon} />
            {link.label}
            {link.live === true && <span className="p-side__live" aria-hidden="true" />}
            {link.count !== undefined && <span className="p-side__count">{link.count}</span>}
          </button>
        ))}
        {hubStatus && (
          <p className={cx('p-side__foot', `p-side__foot--${hubStatus.tone}`)}>
            <b>
              <Icon name={hubStatus.tone === 'success' ? 'shield' : 'warn'} size="sm" />
              {hubStatus.title}
            </b>
            {hubStatus.detail}
          </p>
        )}
      </nav>
      <HeaderBar tools={tools} bell={null} />
      <main className={cx('p-page', variant === 'flush' && 'p-page--flush')}>
        {variant === 'flush' ? (
          children
        ) : (
          <div className={cx('p-page__inner', variant === 'center' && 'p-page__inner--center')}>
            {children}
          </div>
        )}
      </main>
    </div>
  );
}
