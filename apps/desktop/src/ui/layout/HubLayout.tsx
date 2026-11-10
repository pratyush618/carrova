import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Icon } from '../Icon/Icon';
import { useChrome } from './chrome';
import './layouts.css';

/** The Carrova wordmark; a button home when `onClick` is given. */
export function Wordmark({ onClick }: { readonly onClick?: () => void }) {
  const { t } = useTranslation();
  const content = (
    <>
      <span className="p-wordmark__mark">
        <Icon name="swap" />
      </span>
      {t('brand.name')}
    </>
  );
  return onClick ? (
    <button type="button" className="p-wordmark" onClick={onClick}>
      {content}
    </button>
  ) : (
    <span className="p-wordmark">{content}</span>
  );
}

export interface HubLayoutProps {
  /** `center` = content centred in the page (Home); `page` = a reading column. */
  readonly variant?: 'page' | 'center';
  readonly children: ReactNode;
}

/** Places: a quiet top bar with the main navigation, then the page. */
export function HubLayout({ variant = 'page', children }: HubLayoutProps) {
  const { t } = useTranslation();
  const { hubLinks, onHome } = useChrome();
  return (
    <>
      <header className="p-topbar">
        <Wordmark onClick={onHome} />
        <nav className="p-nav" aria-label={t('nav.label')}>
          {hubLinks.map((link) => (
            <button
              key={link.id}
              type="button"
              className="p-nav__link"
              aria-current={link.current ? 'page' : undefined}
              onClick={link.onSelect}
            >
              <Icon name={link.icon} size="sm" />
              <span>{link.label}</span>
            </button>
          ))}
        </nav>
      </header>
      {variant === 'center' ? (
        <main className="p-hub-center">{children}</main>
      ) : (
        <main className="p-page">
          <div className="p-page__inner">{children}</div>
        </main>
      )}
      <span />
    </>
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
