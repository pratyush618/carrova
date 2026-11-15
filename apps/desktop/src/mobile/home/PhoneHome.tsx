import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { deviceIcon } from '../../screens/devices/model';
import { AppGlyph, Button, List, Row } from '../../ui';
import { PhonePage } from '../PhonePage';

/** Home: the linked computers. */
export function PhoneHome({
  devices,
  onAdd,
}: {
  readonly devices: readonly LinkedDeviceView[];
  readonly onAdd: () => void;
}) {
  const { t } = useTranslation();
  return (
    <PhonePage
      title={t('mobile.home.title')}
      actions={
        <Button kind="quiet" size="sm" icon="laptop" onClick={onAdd}>
          {t('mobile.welcome.add')}
        </Button>
      }
    >
      <List label={t('mobile.home.computers')}>
        {devices.map((device) => (
          <Row
            key={device.id}
            off={device.state !== 'linked'}
            lead={<AppGlyph glyph={{ icon: deviceIcon(device.os) }} size="sm" />}
            title={device.name}
            sub={t(`ecosystem.status.${device.state}`)}
          />
        ))}
      </List>
    </PhonePage>
  );
}
