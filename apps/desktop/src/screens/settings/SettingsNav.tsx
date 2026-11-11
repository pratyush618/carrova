import { useTranslation } from 'react-i18next';
import { Icon, Search } from '../../ui';
import { SECTIONS, type SettingsSection } from './sections';

/** Settings' sidebar: back to the page it was opened from, search, and the sections. */
export function SettingsNav({
  section,
  query,
  updateReady,
  onQuery,
  onSection,
  onBack,
}: {
  /** The open section; `null` while search results are shown. */
  readonly section: SettingsSection | null;
  readonly query: string;
  /** A newer Carrova is waiting: Updates carries a count. */
  readonly updateReady: boolean;
  readonly onQuery: (query: string) => void;
  readonly onSection: (section: SettingsSection) => void;
  readonly onBack: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="p-side s-settings-side">
      <button
        type="button"
        className="p-side__link"
        aria-label={t('settings.nav.back')}
        onClick={onBack}
      >
        <Icon name="back" />
        {t('nav.settings')}
      </button>
      <Search
        label={t('settings.nav.search')}
        placeholder={t('settings.nav.search')}
        value={query}
        onChange={onQuery}
      />
      <nav aria-label={t('settings.nav.label')} className="s-settings-sections">
        {SECTIONS.map(({ id, icon }) => (
          <button
            key={id}
            type="button"
            className="p-side__link"
            aria-current={id === section ? 'page' : undefined}
            onClick={() => {
              onSection(id);
            }}
          >
            <Icon name={icon} />
            {t(`settings.sections.${id}`)}
            {id === 'updates' && updateReady && <span className="p-side__count">1</span>}
          </button>
        ))}
      </nav>
    </div>
  );
}
