import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Button, Icon, LinkRow, Row, Segmented, Toggle, type SegmentOption } from '../../ui';
import { Group } from './Group';
import type { Appearance, MotionChoice, SettingsIntents, SettingsModel } from './model';
import { RecipesRow } from './RecipesRow';
import './settings.css';

interface Props {
  readonly model: SettingsModel;
  readonly on: SettingsIntents;
}

const APPEARANCES: readonly Appearance[] = ['system', 'light', 'dark'];
const MOTIONS: readonly MotionChoice[] = ['system', 'reduce'];

export function LookGroup({ model, on }: Props) {
  const { t } = useTranslation();
  const themes: SegmentOption<Appearance>[] = APPEARANCES.map((v) => ({
    value: v,
    label: t(`settings.device.themes.${v}`),
  }));
  const motions: SegmentOption<MotionChoice>[] = MOTIONS.map((v) => ({
    value: v,
    label: t(`settings.device.motions.${v}`),
  }));
  return (
    <Group title={t('settings.device.group')}>
      <Row
        title={t('settings.device.appearance')}
        sub={t('settings.device.appearanceSub')}
        trail={
          <Segmented
            label={t('settings.device.appearance')}
            options={themes}
            value={model.appearance}
            onChange={on.onAppearance}
          />
        }
      />
      <Row
        title={t('settings.device.motion')}
        sub={t('settings.device.motionSub')}
        trail={
          <Segmented
            label={t('settings.device.motion')}
            options={motions}
            value={model.motion}
            onChange={on.onMotion}
          />
        }
      />
    </Group>
  );
}

export function UpdatesGroup({ model, on }: Props) {
  const { t } = useTranslation();
  return (
    <Group title={t('settings.updates.group')}>
      {model.recipes && <RecipesRow recipes={model.recipes} onCheck={on.onCheckRecipes} />}
      <Row
        title={t('settings.updates.autoRecipes')}
        sub={t('settings.updates.autoRecipesSub')}
        trail={
          <Toggle
            on={model.autoCheckRecipes}
            label={t('settings.updates.autoRecipes')}
            onChange={on.onAutoCheckRecipes}
          />
        }
      />
      <Row
        title={t('settings.updates.autoApp')}
        sub={t('settings.updates.autoAppSub')}
        trail={
          <Toggle
            on={model.autoCheckApp}
            label={t('settings.updates.autoApp')}
            onChange={on.onAutoCheckApp}
          />
        }
      />
      <LinkRow
        lead={<Icon name="receive" />}
        title={t('settings.more.updates')}
        sub={
          model.update
            ? t('settings.more.updateReady', { version: model.update.version })
            : t('settings.more.checkApp')
        }
        trail={<Icon name="chev" />}
        onClick={on.onUpdates}
      />
    </Group>
  );
}

export function PrivacyGroup({ model, on }: Props) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <Group title={t('settings.privacy.group')}>
      <Row
        title={t('settings.privacy.reports')}
        sub={t('settings.privacy.reportsSub')}
        trail={
          <Toggle
            on={model.errorReports}
            label={t('settings.privacy.reports')}
            onChange={on.onErrorReports}
          />
        }
      />
      {model.reports.map((report) => (
        <LinkRow
          key={report.id}
          lead={<Icon name="warn" />}
          title={report.headline}
          sub={format.day(report.when, model.now)}
          trail={<Icon name="chev" />}
          onClick={() => {
            on.onOpenReport(report.id);
          }}
        />
      ))}
      {model.reports.length > 0 && (
        <Row
          title={t('settings.privacy.stored', { count: model.reports.length })}
          trail={
            <Button kind="quiet" size="sm" onClick={on.onClearReports}>
              {t('settings.privacy.clear')}
            </Button>
          }
        />
      )}
    </Group>
  );
}

export function MoreGroup({ on }: Props) {
  const { t } = useTranslation();
  return (
    <Group title={t('settings.more.group')}>
      <LinkRow
        lead={<Icon name="info" />}
        title={t('settings.more.about')}
        trail={<Icon name="chev" />}
        onClick={on.onAbout}
      />
    </Group>
  );
}
