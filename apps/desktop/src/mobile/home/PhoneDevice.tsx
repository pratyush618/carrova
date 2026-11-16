import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { DeviceDetails } from '../../screens/devices/DevicePage';
import { FEATURE_ICON, type DeviceIntents } from '../../screens/devices/model';
import { Group } from '../../screens/settings/Group';
import { AppGlyph, LinkRow } from '../../ui';
import '../../screens/devices/devices.css';
import '../../screens/settings/settings.css';
import { PhonePage } from '../PhonePage';

/** A phone feature with its own screen on this phone. */
export type PhoneScreen = 'notifications' | 'messages' | 'calls';

const SCREENS: readonly PhoneScreen[] = ['notifications', 'messages', 'calls'];

/**
 * One computer's page: the desktop's device details in a phone page, then
 * the phone features set up on this phone.
 */
export function PhoneDevice({
  device,
  failed,
  onBack,
  onScreen,
  on,
}: {
  readonly device: LinkedDeviceView;
  /** The engine refused the last feature change. */
  readonly failed: boolean;
  readonly onBack: () => void;
  readonly onScreen: (screen: PhoneScreen) => void;
  readonly on: DeviceIntents;
}) {
  const { t } = useTranslation();
  return (
    <PhonePage onBack={onBack}>
      <DeviceDetails device={device} failed={failed} on={on} />
      <Group title={t('mobile.phone.group')}>
        {SCREENS.map((screen) => (
          <LinkRow
            key={screen}
            lead={<AppGlyph glyph={{ icon: FEATURE_ICON[screen] }} size="sm" />}
            title={t(`mobile.phone.${screen}`)}
            sub={t(`mobile.phone.${screen}Sub`, { name: device.name })}
            onClick={() => {
              onScreen(screen);
            }}
          />
        ))}
      </Group>
    </PhonePage>
  );
}
