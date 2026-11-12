import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { HubLayout, LinkRow, Marked } from '../../ui';
import { AlertsGroup } from './AlertsGroup';
import { BackgroundGroup } from './BackgroundGroup';
import { Group } from './Group';
import type { SettingsIntents, SettingsModel } from './model';
import { MovesGroup } from './MovesGroup';
import { NearbyGroup } from './NearbyGroup';
import { searchSettings } from './search';
import type { SettingsSection } from './sections';
import { SettingsNav } from './SettingsNav';
import { LookGroup, MoreGroup, PrivacyGroup, SendingGroup, UpdatesGroup } from './SettingsGroups';

interface Props {
  readonly model: SettingsModel;
  readonly on: SettingsIntents;
}

/** What each section shows. */
const BODY: Record<SettingsSection, (props: Props) => ReactNode> = {
  general: (p) => (
    <>
      <LookGroup {...p} />
      <BackgroundGroup {...p} />
    </>
  ),
  sending: (p) => <SendingGroup {...p} />,
  nearby: (p) => <NearbyGroup {...p} />,
  notifications: (p) => <AlertsGroup {...p} />,
  moves: (p) => <MovesGroup {...p} />,
  privacy: (p) => <PrivacyGroup {...p} />,
  updates: (p) => <UpdatesGroup {...p} />,
  about: (p) => <MoreGroup {...p} />,
};

/** Settings: the sidebar lists the sections, search covers them all, the page shows one. */
export function SettingsScreen({
  section,
  model,
  on,
}: Props & { readonly section: SettingsSection }) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  const searching = query.trim() !== '';
  const hits = searchSettings(query, t, model);
  const open = (target: SettingsSection) => {
    setQuery('');
    on.onSection(target);
  };
  return (
    <HubLayout
      side={
        <SettingsNav
          section={searching ? null : section}
          query={query}
          updateReady={model.update !== null}
          onQuery={setQuery}
          onSection={open}
          onBack={on.onBack}
        />
      }
    >
      {searching ? (
        <Group
          title={
            hits.length === 0
              ? t('settings.results.none', { query: query.trim() })
              : t('settings.results.found', { count: hits.length, query: query.trim() })
          }
        >
          {hits.map(({ section: target, row, title }) => (
            <LinkRow
              key={row}
              title={<Marked text={title} query={query} />}
              sub={t(`settings.sections.${target}`)}
              onClick={() => {
                open(target);
              }}
            />
          ))}
        </Group>
      ) : (
        BODY[section]({ model, on })
      )}
    </HubLayout>
  );
}
