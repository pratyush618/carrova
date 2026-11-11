import { useTranslation } from 'react-i18next';
import { AppGlyph } from '../../ui';
import { cx } from '../../ui/cx';
import type { CatalogEntry } from './model';
import { TierBadge } from './TierBadge';

/** The name with the first match of the search text marked. */
function Name({ name, query }: { readonly name: string; readonly query: string }) {
  const needle = query.trim();
  const at = needle === '' ? -1 : name.toLocaleLowerCase().indexOf(needle.toLocaleLowerCase());
  if (at < 0) return name;
  return (
    <>
      {name.slice(0, at)}
      <mark className="s-hit">{name.slice(at, at + needle.length)}</mark>
      {name.slice(at + needle.length)}
    </>
  );
}

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
          <Name name={entry.name} query={query} />
        </span>
        {entry.tier !== 'verified' && <TierBadge tier={entry.tier} />}
        <span className="s-app__away">{entry.here ? '' : t('apps.notHere')}</span>
      </button>
    </li>
  );
}
