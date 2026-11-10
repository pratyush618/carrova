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
  readonly children: ReactNode;
}

/** Places: the sidebar (main navigation, bundle state at its foot), then the page. */
export function HubLayout({ variant = 'page', children }: HubLayoutProps) {
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

/** A hub page's heading row: the title, and optional controls at the end. */
export function PageHead({ title, aside }: { readonly title: string; readonly aside?: ReactNode }) {
  return (
    <div className="p-page__head">
      <h1 className="p-title" tabIndex={-1} data-autofocus>
        {title}
      </h1>
      {aside}
    </div>
  );
}
