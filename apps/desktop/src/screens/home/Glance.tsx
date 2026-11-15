import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { AppGlyph, List, LinkRow } from '../../ui';
import { deviceIcon } from '../devices/model';

/** Each linked device once, with the one fact that matters now (linked or away); opens its page. */
export function Glance({
  devices,
  onOpen,
}: {
  readonly devices: readonly LinkedDeviceView[];
  readonly onOpen: (id: string) => void;
}) {
  const { t } = useTranslation();
  return (
    <List label={t('ecosystem.glance')}>
      {devices.map((device) => (
        <LinkRow
          key={device.id}
          off={device.state === 'away'}
          lead={
            <AppGlyph
              glyph={{ icon: deviceIcon(device.os) }}
              tone={device.state === 'away' ? 'neutral' : 'accent'}
            />
          }
          title={device.name}
          sub={t(`ecosystem.status.${device.state}`)}
          onClick={() => {
            onOpen(device.id);
          }}
        />
      ))}
    </List>
  );
}
