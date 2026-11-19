import { useTranslation } from 'react-i18next';
import { useControllers } from '../app/controllers/Controllers';
import { usePort } from '../engine/port';
import type { VisibilityChoice } from '../engine/types';
import { useClipSettings } from '../flows/ecosystem/useClipSettings';
import { current } from '../flows/remote';
import { useFormat } from '../i18n/useFormat';
import { VISIBILITY_CHOICES, VISIBILITY_KEY } from '../screens/nearby/model';
import { deviceIcon } from '../screens/devices/model';
import { ClipboardGroup } from '../screens/settings/EcosystemGroups';
import { Group } from '../screens/settings/Group';
import { AppGlyph, Button, Icon, LinkRow, Row, Segmented, Toggle } from '../ui';
import { PhonePage } from './PhonePage';
import '../screens/settings/settings.css';

/**
 * The phone's Settings: this phone, its computers, the clipboard, app
 * updates (from GitHub releases) and About.
 */
export function PhoneSettings({
  on,
}: {
  readonly on: {
    readonly onOpen: (id: string) => void;
    readonly onAdd: () => void;
    readonly onActivity: () => void;
    /** Open the update on offer. */
    readonly onUpdate: () => void;
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
      <UpdatesGroup onUpdate={on.onUpdate} />
      <Group title={t('mobile.settings.about')}>
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

/** Updates: the one on offer, this version, Check now, and whether mobile data may download. */
function UpdatesGroup({ onUpdate }: { readonly onUpdate: () => void }) {
  const { t } = useTranslation();
  const format = useFormat();
  const { settings, update } = useControllers();
  const offer = current(update.check);
  const saved = settings.settings?.app_update;
  const checked =
    update.checkedAt ?? (saved?.last_checked == null ? null : saved.last_checked * 1000);
  const mobileData = saved?.mobile_data ?? false;
  return (
    <Group title={t('mobile.settings.updates')} sub={t('mobile.settings.updatesFoot')}>
      {offer?.status === 'available' && (
        <LinkRow
          lead={<AppGlyph glyph={{ icon: 'receive' }} size="sm" />}
          title={t('mobile.settings.ready', { version: offer.version })}
          sub={t('mobile.settings.readySub')}
          trail={<Icon name="chev" />}
          onClick={onUpdate}
        />
      )}
      {settings.about !== null && (
        <Row
          title={t('mobile.settings.version', { version: settings.about.app_version })}
          sub={t('mobile.settings.license')}
        />
      )}
      <Row
        title={t('mobile.settings.check')}
        sub={
          checked === null
            ? t('mobile.settings.neverChecked')
            : t('mobile.settings.lastChecked', {
                when: `${format.day(new Date(checked), new Date(), false)}, ${format.time(new Date(checked))}`,
              })
        }
        trail={
          <Button
            size="sm"
            busy={update.check.status === 'loading'}
            disabled={update.check.status === 'loading'}
            onClick={update.runCheck}
          >
            {t('mobile.settings.checkNow')}
          </Button>
        }
      />
      <Row
        title={t('mobile.settings.mobileData')}
        sub={t(mobileData ? 'mobile.settings.mobileDataOn' : 'mobile.settings.mobileDataOff')}
        trail={
          <Toggle
            on={mobileData}
            label={t('mobile.settings.mobileData')}
            onChange={(on) => {
              settings.save({ app_update: { mobile_data: on } });
            }}
          />
        }
      />
    </Group>
  );
}
