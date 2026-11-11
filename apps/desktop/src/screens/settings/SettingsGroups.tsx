import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Button, Icon, LinkRow, Row, Segmented, Toggle, type SegmentOption } from '../../ui';
import { Group } from './Group';
import type { Appearance, MotionChoice, SettingsIntents, SettingsModel } from './model';
import { RecipesRow } from './RecipesRow';
import { ROW_TITLE } from './sections';
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
        title={t(ROW_TITLE.theme)}
        sub={t('settings.device.appearanceSub')}
        trail={
          <Segmented
            label={t(ROW_TITLE.theme)}
            options={themes}
            value={model.appearance}
            onChange={on.onAppearance}
          />
        }
      />
      <Row
        title={t(ROW_TITLE.motion)}
        sub={t('settings.device.motionSub')}
        trail={
          <Segmented
            label={t(ROW_TITLE.motion)}
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
      {model.recipes && (
        <RecipesRow
          recipes={model.recipes}
          onCheck={on.onCheckRecipes}
          onInstall={on.onInstallRecipes}
        />
      )}
      <Row
        title={t(ROW_TITLE.autoRecipes)}
        sub={t('settings.updates.autoRecipesSub')}
        trail={
          <Toggle
            on={model.autoCheckRecipes}
            label={t(ROW_TITLE.autoRecipes)}
            onChange={on.onAutoCheckRecipes}
          />
        }
      />
      <Row
        title={t(ROW_TITLE.autoApp)}
        sub={t('settings.updates.autoAppSub')}
        trail={
          <Toggle
            on={model.autoCheckApp}
            label={t(ROW_TITLE.autoApp)}
            onChange={on.onAutoCheckApp}
          />
        }
      />
      <LinkRow
        lead={<Icon name="receive" />}
        title={t(ROW_TITLE.appUpdate)}
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

/** How sends start (the Send screen's mode, "Send with Carrova" in the file manager) and what they bring. */
export function SendingGroup({ model, on }: Props) {
  const { t } = useTranslation();
  const entries = model.fileManager;
  return (
    <>
      <Group title={t('settings.sending.group')}>
        <Row
          title={t(ROW_TITLE.manual)}
          sub={t('settings.sending.manualSub')}
          trail={
            <Toggle
              on={model.startInManual}
              label={t(ROW_TITLE.manual)}
              onChange={on.onStartInManual}
            />
          }
        />
        {entries && (
          <Row
            title={t(ROW_TITLE.fileManager)}
            sub={
              <>
                {t('settings.sending.fileManagerSub')}
                {entries.skipped.map((line) => (
                  <span key={line} className="s-settings-note">
                    {line}
                  </span>
                ))}
                {entries.failed && (
                  <span role="status" className="s-settings-note">
                    {t('settings.sending.fileManagerFailed')}
                  </span>
                )}
              </>
            }
            trail={
              <Toggle
                on={entries.on}
                label={t(ROW_TITLE.fileManager)}
                onChange={on.onFileManager}
              />
            }
          />
        )}
      </Group>
      <Group title={t('settings.sending.includes')}>
        <Row
          title={t(ROW_TITLE.caches)}
          sub={t('settings.moves.cachesSub')}
          trail={
            <Toggle
              on={model.moveDefaults.include_caches}
              label={t(ROW_TITLE.caches)}
              onChange={(include_caches) => {
                on.onMoveDefaults({ include_caches });
              }}
            />
          }
        />
      </Group>
    </>
  );
}

export function PrivacyGroup({ model, on }: Props) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <Group title={t('settings.privacy.group')}>
      <Row
        title={t(ROW_TITLE.errorReports)}
        sub={t('settings.privacy.reportsSub')}
        trail={
          <Toggle
            on={model.errorReports}
            label={t(ROW_TITLE.errorReports)}
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
        title={t(ROW_TITLE.about)}
        trail={<Icon name="chev" />}
        onClick={on.onAbout}
      />
    </Group>
  );
}
