import { useTranslation } from 'react-i18next';
import { Row, Toggle } from '../../ui';
import { Group } from './Group';
import type { SettingsIntents, SettingsModel } from './model';
import { ROW_TITLE } from './sections';

/** Whether Carrova stays in the tray while the window is closed, and how little it then uses. */
export function BackgroundGroup({
  model: { background },
  on,
}: {
  readonly model: SettingsModel;
  readonly on: SettingsIntents;
}) {
  const { t } = useTranslation();
  const keepTitle = t(ROW_TITLE.keepRunning);
  const loginTitle = t(ROW_TITLE.startAtLogin);
  const leanTitle = t(ROW_TITLE.lean);
  return (
    <Group title={t('settings.background.group')}>
      <Row
        title={keepTitle}
        sub={t('settings.background.keepRunningSub')}
        trail={
          <Toggle
            on={background.keep_running}
            label={keepTitle}
            onChange={(keep_running) => {
              on.onBackground({ keep_running });
            }}
          />
        }
      />
      <Row
        nested
        off={!background.keep_running}
        title={loginTitle}
        sub={t('settings.background.startAtLoginSub')}
        trail={
          <Toggle
            on={background.start_at_login}
            label={loginTitle}
            disabled={!background.keep_running}
            onChange={(start_at_login) => {
              on.onBackground({ start_at_login });
            }}
          />
        }
      />
      <Row
        nested
        off={!background.keep_running}
        title={leanTitle}
        sub={t('settings.background.leanSub')}
        trail={
          <Toggle
            on={background.lean}
            label={leanTitle}
            disabled={!background.keep_running}
            onChange={(lean) => {
              on.onBackground({ lean });
            }}
          />
        }
      />
    </Group>
  );
}
