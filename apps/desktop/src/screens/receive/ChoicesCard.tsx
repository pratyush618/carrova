import { useTranslation } from 'react-i18next';
import { Card, Input, List, Row, Select, Toggle, type SelectOption } from '../../ui';
import { retentionChoices, retentionOf, retentionValue } from '../shared/retention';
import type { ImportChoices } from './model';

/** Import-wide choices, each on its suggestion and each the user's to change. */
export function ChoicesCard({
  choices,
  onChange,
}: {
  readonly choices: ImportChoices;
  readonly onChange: (patch: Partial<ImportChoices>) => void;
}) {
  const { t } = useTranslation();
  const options: SelectOption<string>[] = retentionChoices(choices.retention).map((r) => ({
    value: retentionValue(r),
    label:
      typeof r === 'string'
        ? t(`receive.choices.retentions.${r}`)
        : t('receive.choices.retentions.keep_for', { count: r.keep_for.days }),
  }));
  return (
    <Card title={t('receive.choices.title')}>
      <List kind="plain">
        <Row
          title={t('receive.choices.suffix')}
          sub={t('receive.choices.suffixSub')}
          trail={
            <Input
              mono
              aria-label={t('receive.choices.suffix')}
              defaultValue={choices.keepBothSuffix}
              onBlur={(event) => {
                onChange({ keepBothSuffix: event.target.value });
              }}
            />
          }
        />
        <Row
          title={t('receive.choices.retention')}
          trail={
            <Select
              label={t('receive.choices.retention')}
              options={options}
              value={retentionValue(choices.retention)}
              onChange={(value) => {
                onChange({ retention: retentionOf(value) });
              }}
            />
          }
        />
        <Row
          title={t('receive.choices.trust')}
          sub={t('receive.choices.trustSub')}
          trail={
            <Toggle
              on={choices.importTrust}
              label={t('receive.choices.trust')}
              onChange={(importTrust) => {
                onChange({ importTrust });
              }}
            />
          }
        />
        <Row
          title={t('receive.choices.groupWrite')}
          sub={t('receive.choices.groupWriteSub')}
          trail={
            <Toggle
              on={choices.keepGroupWrite}
              label={t('receive.choices.groupWrite')}
              onChange={(keepGroupWrite) => {
                onChange({ keepGroupWrite });
              }}
            />
          }
        />
      </List>
    </Card>
  );
}
