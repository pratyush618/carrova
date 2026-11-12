import { useTranslation } from 'react-i18next';
import { Button, Icon, LinkRow, Row, Segmented, Toggle, type SegmentOption } from '../../ui';
import type { VisibilityChoice } from '../../engine/types';
import { VISIBILITY_CHOICES, VISIBILITY_KEY } from '../nearby/model';
import { Group } from './Group';
import type { SettingsIntents, SettingsModel } from './model';
import { ROW_TITLE } from './sections';

/** Who can find this computer (the Nearby pill's value), and the devices it remembers. */
export function NearbyGroup({
  model: { nearby, visibility, knownDevices },
  on,
}: {
  readonly model: SettingsModel;
  readonly on: SettingsIntents;
}) {
  const { t } = useTranslation();
  const choices: SegmentOption<VisibilityChoice>[] = VISIBILITY_CHOICES.map((value) => ({
    value,
    label: t(`settings.nearby.choices.${VISIBILITY_KEY[value]}`),
  }));
  const toggle = (
    title: string,
    value: boolean,
    onChange: (value: boolean) => void,
    disabled = false,
  ) => <Toggle on={value} label={title} disabled={disabled} onChange={onChange} />;
  return (
    <>
      <Group title={t('settings.nearby.group')}>
        <Row
          title={t(ROW_TITLE.visibility)}
          sub={t('settings.nearby.visibilitySub')}
          trail={
            <Segmented
              label={t(ROW_TITLE.visibility)}
              options={choices}
              value={visibility}
              onChange={on.onVisibility}
            />
          }
        />
        <Row
          title={t(ROW_TITLE.acceptKnown)}
          sub={t('settings.nearby.acceptKnownSub')}
          trail={toggle(t(ROW_TITLE.acceptKnown), nearby.accept_known, (accept_known) => {
            on.onNearby({ accept_known });
          })}
        />
      </Group>
      <Group title={t('settings.nearby.history')}>
        <Row
          title={t(ROW_TITLE.remember)}
          sub={t('settings.nearby.rememberSub')}
          trail={toggle(t(ROW_TITLE.remember), nearby.remember, (remember) => {
            on.onNearby({ remember });
          })}
        />
        <LinkRow
          title={t(ROW_TITLE.knownDevices)}
          sub={
            knownDevices === 0
              ? t('settings.nearby.knownDevicesNone')
              : t('settings.nearby.knownDevicesSub', { count: knownDevices })
          }
          trail={<Icon name="chev" />}
          onClick={on.onKnownDevices}
        />
        <Row
          nested
          off={!nearby.remember}
          title={t(ROW_TITLE.wolHints)}
          sub={t('settings.nearby.wolHintsSub')}
          trail={toggle(
            t(ROW_TITLE.wolHints),
            nearby.wol_hints,
            (wol_hints) => {
              on.onNearby({ wol_hints });
            },
            !nearby.remember,
          )}
        />
        <Row
          title={t(ROW_TITLE.resetIdentity)}
          sub={t('settings.nearby.resetSub')}
          trail={
            <Button kind="quiet-danger" size="sm" onClick={on.onResetIdentity}>
              {t('settings.nearby.resetAction')}
            </Button>
          }
        />
      </Group>
    </>
  );
}
