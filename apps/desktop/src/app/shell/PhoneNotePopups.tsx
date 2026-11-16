import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useHost } from '../../engine/host';
import { useControllers } from '../controllers/Controllers';
import { useLink } from '../state/EngineState';

/**
 * Each phone notification the engine pops up: in Carrova's own pop-up
 * window, which fetches it by key; while this computer is locked only a
 * system notification naming the phone (the OS keeps those, so never
 * content). Renders nothing.
 */
export function PhoneNotePopups() {
  const { t } = useTranslation();
  const { note } = useLink();
  const { linked } = useControllers();
  const host = useHost();
  // Once per notification, not again when the device list reloads.
  const shownN = useRef(0);
  useEffect(() => {
    if (note === null || note.n === shownN.current) return;
    shownN.current = note.n;
    const name =
      linked.devices.find((d) => d.id === note.device)?.name ?? t('ecosystem.unknownDevice');
    const shown = note.locked
      ? host.notify?.(t('ecosystem.notes.os', { name }))
      : host.showNotePopup?.(
          new URLSearchParams({
            popup: 'note',
            device: note.device,
            key: note.key,
            name,
          }).toString(),
        );
    // Best effort, like the OS's own pop-ups: the note stays on the phone.
    shown?.catch(() => undefined);
  }, [note, linked.devices, host, t]);
  return null;
}
