import { useTranslation } from 'react-i18next';
import {
  Badge,
  Button,
  Card,
  List,
  Muted,
  Path,
  Row,
  Segmented,
  Select,
  Sheet,
  type SegmentOption,
  type SelectOption,
} from '../../ui';
import type { ConflictRule } from '../shared/model';
import { ChoicesCard } from './ChoicesCard';
import type { ChangeGroup, Clash, ReviewDetailsIntents, ReviewDetailsModel } from './model';

const RULES: readonly ConflictRule[] = ['replace', 'keep-both', 'skip'];

/** A per-file choice: one of the rules, or "same as the item". */
type FileChoice = ConflictRule | 'item';

function ClashRow({
  group,
  clash,
  on,
}: {
  readonly group: ChangeGroup;
  readonly clash: Clash;
  readonly on: ReviewDetailsIntents;
}) {
  const { t } = useTranslation();
  const rule = clash.rule ?? group.rule;
  const choices: readonly SelectOption<FileChoice>[] = [
    {
      value: 'item',
      label: t('receive.details.sameAsItem', { rule: t(`receive.details.rule.${group.rule}`) }),
    },
    ...RULES.map((value) => ({ value, label: t(`receive.details.rule.${value}`) })),
  ];
  return (
    <Row
      title={<Path>{clash.display}</Path>}
      sub={
        <>
          {t(`receive.details.exists.${rule}`)}{' '}
          {rule === 'replace' && <Badge>{t('badge.suggested')}</Badge>}
        </>
      }
      trail={
        <Select
          label={t('receive.details.fileRule', { path: clash.display })}
          options={choices}
          value={clash.rule ?? 'item'}
          onChange={(choice) => {
            on.onFileRule(group.id, clash.path, choice === 'item' ? null : choice);
          }}
        />
      }
    />
  );
}

function GroupCard({
  group,
  on,
}: {
  readonly group: ChangeGroup;
  readonly on: ReviewDetailsIntents;
}) {
  const { t } = useTranslation();
  const options: readonly SegmentOption<ConflictRule>[] = RULES.map((value) => ({
    value,
    label: t(`receive.details.rule.${value}`),
  }));
  return (
    <Card
      title={group.name}
      aside={
        group.clashes.length > 0 && (
          <Segmented
            label={t('receive.details.clashes', { app: group.name })}
            options={options}
            value={group.rule}
            onChange={(rule) => {
              on.onRule(group.id, rule);
            }}
          />
        )
      }
    >
      <List kind="plain">
        {group.clashes.map((clash) => (
          <ClashRow key={clash.path} group={group} clash={clash} on={on} />
        ))}
        <Row
          title={t('receive.details.added', { count: group.added })}
          sub={
            <>
              {t('receive.details.addedSub')}{' '}
              {group.remapped && <Badge tone="warning">{t('badge.elsewhere')}</Badge>}
            </>
          }
          trail={
            group.canRemap && (
              <Button
                kind="quiet"
                size="sm"
                onClick={() => {
                  on.onRemap(group.id);
                }}
              >
                {t('receive.details.remap')}
              </Button>
            )
          }
        />
      </List>
    </Card>
  );
}

/** Where "user decides everything" lives in full: per-item and per-file rules, restore elsewhere. */
export function ReviewDetailsSheet({
  model,
  on,
}: {
  readonly model: ReviewDetailsModel;
  readonly on: ReviewDetailsIntents;
}) {
  const { t } = useTranslation();
  return (
    <Sheet
      title={t('receive.details.title')}
      onClose={on.onDone}
      foot={
        <Button kind="primary" onClick={on.onDone}>
          {t('action.done')}
        </Button>
      }
    >
      <Muted>{t('receive.details.intro')}</Muted>
      {model.groups.map((group) => (
        <GroupCard key={group.id} group={group} on={on} />
      ))}
      <ChoicesCard choices={model.choices} onChange={on.onChoices} />
    </Sheet>
  );
}
