import type { LinkedDeviceView } from '../../engine/types';
import { messagesPhone } from '../../flows/phone/messagesPhone';
import type { View } from '../nav/navMachine';
import type { NoteAlert } from '../state/link';

/**
 * Whether `note` needs no pop-up: the phone's SMS app posted it while this
 * window is in front (`focused`) on Messages for that phone, which already
 * shows the text. It still joins the notifications list.
 */
export function quietNote(
  note: NoteAlert,
  view: View,
  devices: readonly LinkedDeviceView[],
  focused: boolean,
): boolean {
  const phone = messagesPhone(devices);
  return (
    note.smsApp &&
    focused &&
    view.kind === 'hub' &&
    view.page === 'messages' &&
    phone?.id === note.device &&
    phone.features.some((f) => f.feature === 'messages' && f.effective !== 'off')
  );
}
