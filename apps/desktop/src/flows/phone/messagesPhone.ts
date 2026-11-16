import type { LinkedDeviceView } from '../../engine/types';

/** The linked phone whose texts Messages shows: the first with Messages on here (away ones too: their texts stay readable). */
export const messagesPhone = (devices: readonly LinkedDeviceView[]): LinkedDeviceView | undefined =>
  devices.find(
    (d) =>
      d.os === 'android' && d.features.some((f) => f.feature === 'messages' && f.local !== 'off'),
  );
