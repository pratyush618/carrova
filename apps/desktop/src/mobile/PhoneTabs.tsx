import { useTranslation } from 'react-i18next';
import { Icon, type IconName } from '../ui';

/** The phone's three places. */
export type PhoneTab = 'home' | 'activity' | 'settings';

const TABS: readonly { readonly tab: PhoneTab; readonly icon: IconName }[] = [
  { tab: 'home', icon: 'home' },
  { tab: 'activity', icon: 'history' },
  { tab: 'settings', icon: 'settings' },
];

/** The bottom navigation between the places. */
export function PhoneTabs({
  current,
  onSelect,
}: {
  readonly current: PhoneTab | null;
  readonly onSelect: (tab: PhoneTab) => void;
}) {
  const { t } = useTranslation();
  return (
    <nav className="m-tabs" aria-label={t('mobile.nav.label')}>
      {TABS.map(({ tab, icon }) => (
        <button
          key={tab}
          type="button"
          className="m-tabs__tab"
          aria-current={tab === current ? 'page' : undefined}
          onClick={() => {
            onSelect(tab);
          }}
        >
          <Icon name={icon} />
          {t(`mobile.nav.${tab}`)}
        </button>
      ))}
    </nav>
  );
}
