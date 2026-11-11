import { useTranslation } from 'react-i18next';
import { AppGlyph, Marked } from '../../ui';
import { cx } from '../../ui/cx';
import type { CatalogEntry } from './model';
import { TierBadge } from './TierBadge';

/** One app in the list. It says only what is unusual: not here, or not reviewed by the team. */
export function AppRow({
  entry,
  query,
  selected,
  onSelect,
}: {
  readonly entry: CatalogEntry;
  readonly query: string;
  readonly selected: boolean;
  readonly onSelect: () => void;
}) {
  const { t } = useTranslation();
  return (
    <li>
      <button
        type="button"
        className={cx('s-app', selected && 'is-sel', !entry.here && 'is-away')}
        aria-pressed={selected}
        onClick={onSelect}
      >
        <AppGlyph glyph={entry.glyph} size="sm" />
        <span className="s-app__name">
          <Marked text={entry.name} query={query} />
        </span>
        {entry.tier !== 'verified' && <TierBadge tier={entry.tier} />}
        <span className="s-app__away">{entry.here ? '' : t('apps.notHere')}</span>
      </button>
    </li>
  );
}
