import type { LinkedDeviceView } from '../../engine/types';
import { DeviceDetails } from '../../screens/devices/DevicePage';
import type { DeviceIntents } from '../../screens/devices/model';
import '../../screens/devices/devices.css';
import '../../screens/settings/settings.css';
import { PhonePage } from '../PhonePage';

/** One computer's page: the desktop's device details in a phone page. */
export function PhoneDevice({
  device,
  failed,
  onBack,
  on,
}: {
  readonly device: LinkedDeviceView;
  /** The engine refused the last feature change. */
  readonly failed: boolean;
  readonly onBack: () => void;
  readonly on: DeviceIntents;
}) {
  return (
    <PhonePage onBack={onBack}>
      <DeviceDetails device={device} failed={failed} on={on} />
    </PhonePage>
  );
}
