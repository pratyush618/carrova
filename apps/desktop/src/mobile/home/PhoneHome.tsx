import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { deviceIcon } from '../../screens/devices/model';
import { AppGlyph, Button, Icon, LinkRow, List } from '../../ui';
import { readyFor } from '../model';
import { PhonePage } from '../PhonePage';

/** Home: the linked computers first, then Send clipboard (to the first one it reaches). */
export function PhoneHome({
  devices,
  on,
}: {
  readonly devices: readonly LinkedDeviceView[];
  readonly on: {
    readonly onAdd: () => void;
    readonly onOpen: (id: string) => void;
    readonly onSendClipboard: () => void;
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
