import { useTranslation } from 'react-i18next';
import type { ConflictRule } from '../../engine/types';
import { Row, Segmented, type SegmentOption } from '../../ui';
import { retentionChoices, retentionOf, retentionValue } from '../shared/retention';
import { Group } from './Group';
import type { SettingsIntents, SettingsModel } from './model';
import { ROW_TITLE } from './sections';

const RULES: readonly ConflictRule[] = ['replace', 'keep-both', 'skip'];

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
  const rules: SegmentOption<ConflictRule>[] = RULES.map((value) => ({
    value,
    label: t(`receive.details.rule.${value}`),
  }));
  const retentions: SegmentOption<string>[] = retentionChoices(defaults.retention).map((r) => ({
    value: retentionValue(r),
    label:
      typeof r === 'string'
        ? t(`settings.moves.retentions.${r}`)
        : t('settings.moves.retentions.keep_for', { count: r.keep_for.days }),
  }));
  return (
    <Group title={t('settings.moves.group')} sub={t('settings.moves.sub')}>
      <Row
        title={t(ROW_TITLE.conflictRule)}
        trail={
          <Segmented
            label={t(ROW_TITLE.conflictRule)}
            options={rules}
            value={defaults.conflict_rule}
            onChange={(conflict_rule) => {
              on.onMoveDefaults({ conflict_rule });
            }}
          />
        }
      />
      <Row
        title={t(ROW_TITLE.retention)}
        trail={
          <Segmented
            label={t(ROW_TITLE.retention)}
            options={retentions}
            value={retentionValue(defaults.retention)}
            onChange={(value) => {
              on.onMoveDefaults({ retention: retentionOf(value) });
            }}
          />
        }
      />
    </Group>
  );
}
