import { HubLayout } from '../../ui';
import type { SettingsIntents, SettingsModel } from './model';
import { MovesGroup } from './MovesGroup';
import { LookGroup, MoreGroup, PrivacyGroup, SendingGroup, UpdatesGroup } from './SettingsGroups';

/** One quiet page: appearance, updates (never installed without asking), sending, moves, privacy, about. */
export function SettingsScreen({
  model,
  on,
}: {
  readonly model: SettingsModel;
  readonly on: SettingsIntents;
}) {
  return (
    <HubLayout>
      <LookGroup model={model} on={on} />
      <UpdatesGroup model={model} on={on} />
      <SendingGroup model={model} on={on} />
      <MovesGroup model={model} on={on} />
      <PrivacyGroup model={model} on={on} />
      <MoreGroup model={model} on={on} />
    </HubLayout>
  );
}
