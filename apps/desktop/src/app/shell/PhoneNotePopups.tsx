import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { useHost } from '../../engine/host';
import { useControllers } from '../controllers/Controllers';
import type { View } from '../nav/navMachine';
import { useLink } from '../state/EngineState';
import { quietNote } from './quietNote';

/**
 * Each phone notification the engine pops up: in Carrova's own pop-up
 * window, which fetches it by key; while this computer is locked only a
 * system notification naming the phone (the OS keeps those, so never
 * content). None for a text while `view`, in front, already shows it
 * (`quietNote`). Renders nothing.
 */
export function PhoneNotePopups({ view }: { readonly view: View }) {
  const { t } = useTranslation();
  const { note } = useLink();
  const { linked } = useControllers();
  const host = useHost();
  // Once per notification, not again when the device list reloads.
  const shownN = useRef(0);
  useEffect(() => {
    if (note === null || note.n === shownN.current) return;
    shownN.current = note.n;
    const focused = document.hasFocus() && document.visibilityState === 'visible';
    if (quietNote(note, view, linked.devices, focused)) return;
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
  }, [note, view, linked.devices, host, t]);
  return null;
}
