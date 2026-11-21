import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { deviceIcon } from '../../screens/devices/model';
import { AppGlyph, Banner, Button, Icon, LinkRow, List } from '../../ui';
import { readyFor } from '../model';
import { PhonePage } from '../PhonePage';

/**
 * Home: a Carrova update on offer, if any, then the linked computers, then Send clipboard
 * (to the first one it reaches).
 */
export function PhoneHome({
  devices,
  update,
  on,
}: {
  readonly devices: readonly LinkedDeviceView[];
  /** The version a check found newer; `null` if none. */
  readonly update: string | null;
  readonly on: {
    readonly onAdd: () => void;
    readonly onOpen: (id: string) => void;
    readonly onSendClipboard: () => void;
    readonly onUpdate: () => void;
  };
}) {
  const { t } = useTranslation();
  const [first] = readyFor(devices, 'clipboard');
  return (
    <PhonePage
      title={t('mobile.home.title')}
      actions={
        <Button kind="quiet" size="sm" icon="laptop" onClick={on.onAdd}>
          {t('mobile.welcome.add')}
        </Button>
      }
    >
      {update !== null && (
        <Banner
          tone="accent"
          icon="receive"
          title={t('mobile.update.banner', { version: update })}
          body={t('mobile.update.bannerSub')}
          action={
            <Button kind="primary" size="sm" onClick={on.onUpdate}>
              {t('mobile.update.view')}
            </Button>
          }
        />
      )}
      <List label={t('mobile.home.computers')}>
        {devices.map((device) => (
          <LinkRow
            key={device.id}
            off={device.state !== 'linked'}
            lead={<AppGlyph glyph={{ icon: deviceIcon(device.os) }} size="sm" />}
            title={device.name}
            sub={t(`ecosystem.status.${device.state}`)}
            trail={<Icon name="chev" />}
            onClick={() => {
              on.onOpen(device.id);
            }}
          />
        ))}
      </List>
      <Button
        kind="primary"
        size="lg"
        icon="copy"
        disabled={first === undefined}
        onClick={on.onSendClipboard}
      >
        {t('mobile.home.sendClipboard')}
      </Button>
      <p className="m-note">
        {first === undefined
          ? t('mobile.home.noneNearby')
          : t('mobile.home.to', { name: first.name })}
      </p>
    </PhonePage>
  );
}
