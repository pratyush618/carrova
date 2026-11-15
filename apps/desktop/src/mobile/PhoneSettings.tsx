import { useTranslation } from 'react-i18next';
import { useControllers } from '../app/controllers/Controllers';
import { usePort } from '../engine/port';
import type { VisibilityChoice } from '../engine/types';
import { useClipSettings } from '../flows/ecosystem/useClipSettings';
import { VISIBILITY_CHOICES, VISIBILITY_KEY } from '../screens/nearby/model';
import { deviceIcon } from '../screens/devices/model';
import { ClipboardGroup } from '../screens/settings/EcosystemGroups';
import { Group } from '../screens/settings/Group';
import { AppGlyph, Icon, LinkRow, Row, Segmented, Toggle } from '../ui';
import { PhonePage } from './PhonePage';
import '../screens/settings/settings.css';

/**
 * The phone's Settings: this phone, its computers, the clipboard and About.
 * No app updates here: a phone updates through a new APK.
 */
export function PhoneSettings({
  on,
}: {
  readonly on: {
    readonly onOpen: (id: string) => void;
    readonly onAdd: () => void;
    readonly onActivity: () => void;
  };
}) {
  const { t } = useTranslation();
  const { devices, linked, settings } = useControllers();
  const { clip, setClip, failed } = useClipSettings(usePort());
  const about = settings.about;
  const visibility = devices.visibility?.choice ?? null;
  return (
    <PhonePage title={t('mobile.settings.title')}>
      <Group title={t('mobile.settings.phone')}>
        <Row
          title={t('mobile.settings.name')}
          {...(about?.device_name != null && { sub: about.device_name })}
        />
        <Row
          title={t('mobile.settings.visibility')}
          trail={
            <Segmented<VisibilityChoice>
              label={t('mobile.settings.visibility')}
              options={VISIBILITY_CHOICES.map((value) => ({
                value,
                label: t(`settings.nearby.choices.${VISIBILITY_KEY[value]}`),
              }))}
              value={visibility}
              onChange={devices.setVisibility}
            />
          }
        />
      </Group>
      <Group title={t('mobile.settings.computers')}>
        {linked.devices.map((device) => (
          <LinkRow
            key={device.id}
            lead={<AppGlyph glyph={{ icon: deviceIcon(device.os) }} size="sm" />}
            title={device.name}
            sub={t(`ecosystem.status.${device.state}`)}
            trail={<Icon name="chev" />}
            onClick={() => {
              on.onOpen(device.id);
            }}
          />
        ))}
        <LinkRow title={t('mobile.welcome.add')} trail={<Icon name="chev" />} onClick={on.onAdd} />
      </Group>
      <ClipboardGroup
        clip={clip}
        clipFailed={failed}
        on={{ onClip: setClip, onClipHistory: on.onActivity }}
      />
      <Group title={t('mobile.settings.about')}>
        {about !== null && (
          <Row
            title={t('mobile.settings.version', { version: about.app_version })}
            sub={t('mobile.settings.license')}
          />
        )}
        <Row
          title={t('mobile.settings.reports')}
          sub={t('mobile.settings.reportsSub')}
          trail={
            <Toggle
              on={settings.consent === 'allowed'}
              label={t('mobile.settings.reports')}
              onChange={settings.setConsent}
            />
          }
        />
      </Group>
    </PhonePage>
  );
}
