import type { Feature } from '../../bindings/Feature';
import type { LinkedDeviceView } from '../../engine/types';

/** The first linked phone with `feature` on here (away ones too: what they shared stays readable). */
const phoneWith = (
  devices: readonly LinkedDeviceView[],
  feature: Feature,
): LinkedDeviceView | undefined =>
  devices.find(
    (d) => d.os === 'android' && d.features.some((f) => f.feature === feature && f.local !== 'off'),
  );

/** The linked phone whose texts Messages shows. */
export const messagesPhone = (devices: readonly LinkedDeviceView[]): LinkedDeviceView | undefined =>
  phoneWith(devices, 'messages');

/** The linked phone whose calls Calls shows. */
export const callsPhone = (devices: readonly LinkedDeviceView[]): LinkedDeviceView | undefined =>
  phoneWith(devices, 'calls');
