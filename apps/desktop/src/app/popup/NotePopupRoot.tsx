import { isTauri } from '@tauri-apps/api/core';
import { useCallback, useEffect, useState } from 'react';
import type { NoteAction } from '../../bindings/NoteAction';
import type { NotesView } from '../../bindings/NotesView';
import { PortProvider, usePort } from '../../engine/port';
import type { CarrovaPort } from '../../engine/types';
import { PreferencesProvider } from '../../preferences/preferences';
import { NotePopup, type NotePopupModel } from '../../screens/phone/notes/NotePopup';

/** What the main window asked this pop-up to show (`PhoneNotePopups`). */
export interface NotePopupTarget {
  readonly device: string;
  readonly key: string;
  /** The phone's name. */
  readonly name: string;
}

/** The pop-up's target from its window's query (`?popup=note&device=…&key=…&name=…`), if it is one. */
export function notePopupTarget(search: string): NotePopupTarget | null {
  const query = new URLSearchParams(search);
  const [device, key, name] = [query.get('device'), query.get('key'), query.get('name')];
  return query.get('popup') === 'note' && device !== null && key !== null && name !== null
    ? { device, key, name }
    : null;
}

/** Close this pop-up window (in a browser: just stop showing it). */
async function closeWindow(): Promise<void> {
  if (!isTauri()) return;
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  await getCurrentWindow().close();
}

/** The root of a phone-notification pop-up window. */
export function NotePopupRoot({
  port,
  target,
}: {
  readonly port: CarrovaPort;
  readonly target: NotePopupTarget;
}) {
  return (
    <PreferencesProvider>
      <PortProvider port={port}>
        <NotePopupRoute target={target} />
      </PortProvider>
    </PreferencesProvider>
  );
}

/** The model for `view`; `null` once the notification is gone (nothing to show). */
function popupModel(view: NotesView, target: NotePopupTarget): NotePopupModel | null {
  if (view.kind === 'locked') return { kind: 'locked', phone: target.name, count: view.count };
  const note = view.notes.find((n) => n.key === target.key);
  return note === undefined ? null : { kind: 'note', phone: target.name, note };
}

/**
 * Fetches the notification by key (content never travels in the window's
 * address), acts on it on the phone, and closes once an action is done.
 */
export function NotePopupRoute({ target }: { readonly target: NotePopupTarget }) {
  const port = usePort();
  const [view, setView] = useState<NotesView | null>(null);
  const [problem, setProblem] = useState<'gone' | 'failed' | null>(null);
  const [closed, setClosed] = useState(false);
  const close = useCallback(() => {
    setClosed(true);
    void closeWindow().catch(() => undefined);
  }, []);
  const { device } = target;
  useEffect(() => {
    port.phoneNotes(device).then(setView, close);
  }, [port, device, close]);
  const model = view === null ? null : popupModel(view, target);
  // Gone from the phone before it showed: nothing to pop up.
  const gone = view !== null && model === null;
  useEffect(() => {
    if (gone) void closeWindow().catch(() => undefined);
  }, [gone]);
  if (closed || model === null) return null;
  const act = (action: NoteAction) => {
    setProblem(null);
    port.phoneNoteAction(device, action).then(
      (outcome) => {
        if (outcome === 'done') close();
        else setProblem(outcome);
      },
      () => {
        setProblem('failed');
      },
    );
  };
  const { key } = target;
  return (
    <NotePopup
      model={model}
      problem={problem}
      on={{
        onReply: (text) => {
          act({ kind: 'reply', key, text });
        },
        onDismiss: () => {
          act({ kind: 'dismiss', key });
        },
        onOpen: () => {
          act({ kind: 'open', key });
        },
        onClose: close,
      }}
    />
  );
}
