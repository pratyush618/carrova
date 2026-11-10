import { useTranslation } from 'react-i18next';
import { AppGlyph } from '../../ui';
import type { CatalogEntry } from './model';
import { TierBadge } from './TierBadge';
import './apps.css';

/** A catalogue tile: the same glyph used everywhere, tier as a badge. */
export function AppCard({
  entry,
  onOpen,
}: {
  readonly entry: CatalogEntry;
  readonly onOpen: () => void;
}) {
  const { t } = useTranslation();
  return (
    <li>
      <button type="button" className="s-app-card" onClick={onOpen}>
        <span className="s-app-card__top">
          <AppGlyph glyph={entry.glyph} />
          <TierBadge tier={entry.tier} />
        </span>
        <span className="p-row__title">{entry.name}</span>
        <span className="p-row__sub">
          {entry.installed ? t('apps.installed') : t('apps.notInstalled')}
        </span>
      </button>
    </li>
  );
}
