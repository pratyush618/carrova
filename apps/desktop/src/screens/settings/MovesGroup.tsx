import { useTranslation } from 'react-i18next';
import type { ConflictRule, Retention } from '../../engine/types';
import { Row, Select, Toggle, type SelectOption } from '../../ui';
import { Group } from './Group';
import type { SettingsIntents, SettingsModel } from './model';

const RULES: readonly ConflictRule[] = ['replace', 'keep-both', 'skip'];
const RETENTIONS: readonly Retention[] = ['until_dismissed', 'discard_on_success'];

/** Suggested defaults every new move starts from; each move can change them. */
export function MovesGroup({
  model,
  on,
}: {
  readonly model: SettingsModel;
  readonly on: SettingsIntents;
}) {
  const { t } = useTranslation();
  const defaults = model.moveDefaults;
  const rules: SelectOption<ConflictRule>[] = RULES.map((value) => ({
    value,
    label: t(`receive.details.rule.${value}`),
  }));
  const retentions: SelectOption<Retention>[] = RETENTIONS.map((value) => ({
    value,
    label: t(`receive.choices.retentions.${value}`),
  }));
  return (
    <Group title={t('settings.moves.group')} sub={t('settings.moves.sub')}>
      <Row
        title={t('settings.moves.rule')}
        trail={
          <Select
            label={t('settings.moves.rule')}
            options={rules}
            value={defaults.conflict_rule}
            onChange={(conflict_rule) => {
              on.onMoveDefaults({ conflict_rule });
            }}
          />
        }
      />
      <Row
        title={t('settings.moves.retention')}
        trail={
          <Select
            label={t('settings.moves.retention')}
            options={retentions}
            value={defaults.retention}
            onChange={(retention) => {
              on.onMoveDefaults({ retention });
            }}
          />
        }
      />
      <Row
        title={t('settings.moves.caches')}
        sub={t('settings.moves.cachesSub')}
        trail={
          <Toggle
            on={defaults.include_caches}
            label={t('settings.moves.caches')}
            onChange={(include_caches) => {
              on.onMoveDefaults({ include_caches });
            }}
          />
        }
      />
    </Group>
  );
}
