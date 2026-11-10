import { useTranslation } from 'react-i18next';
import { HubLayout, PageHead } from '../../ui';
import type { SettingsIntents, SettingsModel } from './model';
import { MovesGroup } from './MovesGroup';
import { LookGroup, MoreGroup, PrivacyGroup, UpdatesGroup } from './SettingsGroups';

/** Look, updates (recipes and app, never installed without asking), privacy, about. */
export function SettingsScreen({
  model,
  on,
}: {
  readonly model: SettingsModel;
  readonly on: SettingsIntents;
}) {
  const { t } = useTranslation();
  return (
    <HubLayout>
      <PageHead title={t('settings.title')} />
      <LookGroup model={model} on={on} />
      <MovesGroup model={model} on={on} />
      <UpdatesGroup model={model} on={on} />
      <PrivacyGroup model={model} on={on} />
      <MoreGroup model={model} on={on} />
    </HubLayout>
  );
}
