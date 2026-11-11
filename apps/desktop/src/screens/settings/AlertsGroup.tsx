import { useTranslation } from 'react-i18next';
import type { AlertChannel } from '../../bindings/AlertChannel';
import type { AlertSettings } from '../../bindings/AlertSettings';
import { useFormat } from '../../i18n/useFormat';
import { Row, Segmented, Toggle, type SegmentOption } from '../../ui';
import { Group } from './Group';
import type { PauseChoice, SettingsIntents, SettingsModel } from './model';
import { ROW_TITLE, type SettingRowId } from './sections';

const PAUSES: readonly PauseChoice[] = ['off', 'hour', 'tomorrow'];
const CHANNELS: readonly AlertChannel[] = ['popup_and_bell', 'bell_only'];

/** The kinds of alert, each with its own channel. */
const KINDS = ['requests', 'moves', 'nearby', 'updates'] as const satisfies readonly (SettingRowId &
  keyof AlertSettings)[];

/** What reaches you and how: system notifications, what depends on them, a pause, and a channel per kind. */
export function AlertsGroup({
  model: { alerts, pausedUntil },
  on,
}: {
  readonly model: SettingsModel;
  readonly on: SettingsIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const pauses: SegmentOption<PauseChoice>[] = PAUSES.map((value) => ({
    value,
    label: t(`settings.alerts.pauses.${value}`),
  }));
  const channels: SegmentOption<AlertChannel>[] = CHANNELS.map((value) => ({
    value,
    label: t(`settings.alerts.channels.${value}`),
  }));
  return (
    <>
      <Group title={t('settings.alerts.group')}>
        <Row
          title={t(ROW_TITLE.system)}
          sub={t('settings.alerts.systemSub')}
          trail={
            <Toggle
              on={alerts.system}
              label={t(ROW_TITLE.system)}
              onChange={(system) => {
                on.onAlerts({ system });
              }}
            />
          }
        />
        <Row
          nested
          off={!alerts.system}
          title={t(ROW_TITLE.wakeScreen)}
          sub={t('settings.alerts.wakeScreenSub')}
          trail={
            <Toggle
              on={alerts.wake_screen}
              label={t(ROW_TITLE.wakeScreen)}
              disabled={!alerts.system}
              onChange={(wake_screen) => {
                on.onAlerts({ wake_screen });
              }}
            />
          }
        />
        <Row
          nested
          off={!alerts.system}
          title={t(ROW_TITLE.lockNames)}
          sub={t('settings.alerts.lockNamesSub')}
          trail={
            <Toggle
              on={alerts.names_on_lock_screen}
              label={t(ROW_TITLE.lockNames)}
              disabled={!alerts.system}
              onChange={(names_on_lock_screen) => {
                on.onAlerts({ names_on_lock_screen });
              }}
            />
          }
        />
        <Row
          title={t(ROW_TITLE.pause)}
          sub={
            pausedUntil === null
              ? t('settings.alerts.pauseSub')
              : t('settings.alerts.pausedUntil', { time: format.time(pausedUntil) })
          }
          trail={
            <Segmented
              label={t(ROW_TITLE.pause)}
              options={pauses}
              value={pausedUntil === null ? 'off' : null}
              onChange={on.onPause}
            />
          }
        />
      </Group>
      <Group title={t('settings.alerts.tell')}>
        {KINDS.map((kind) => (
          <Row
            key={kind}
            title={t(ROW_TITLE[kind])}
            trail={
              <Segmented
                label={t(ROW_TITLE[kind])}
                options={channels}
                value={alerts[kind]}
                onChange={(channel) => {
                  on.onAlerts({ [kind]: channel });
                }}
              />
            }
          />
        ))}
      </Group>
    </>
  );
}
