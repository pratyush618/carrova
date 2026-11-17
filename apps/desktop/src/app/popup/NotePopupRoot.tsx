import { isTauri } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState, type FocusEvent } from 'react';
import type { NoteAction } from '../../bindings/NoteAction';
import type { NotesView } from '../../bindings/NotesView';
import { PortProvider, usePort } from '../../engine/port';
import type { CarrovaPort } from '../../engine/types';
import { PreferencesProvider } from '../../preferences/preferences';
import { NotePopup, type NotePopupModel } from '../../screens/phone/notes/NotePopup';
import { useReducedMotion } from '../../ui';
import { cx } from '../../ui/cx';

/** How long a pop-up stays while left alone, then how long it fades out. */
export const NOTE_POPUP_TIMING = { showMs: 5_000, fadeMs: 250 } as const;

/** What the main window asked this pop-up to show (`PhoneNotePopups`). */
export interface NotePopupTarget {
  readonly device: string;
  readonly key: string;
  /** The phone's name when the window opened; the linked device's current name wins. */
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
function popupModel(view: NotesView, key: string, phone: string): NotePopupModel | null {
  if (view.kind === 'locked') return { kind: 'locked', phone, count: view.count };
  const note = view.notes.find((n) => n.key === key);
  return note === undefined ? null : { kind: 'note', phone, note };
}

/**
 * Closes the pop-up once left alone for `showMs`: fades out first unless
 * motion is reduced. Held while the pointer is over it or focus is in it
 * (a reply being typed). Closing only hides it: the note stays unread.
 */
function useAutoDismiss(shown: boolean, close: () => void) {
  const reduce = useReducedMotion();
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [fading, setFading] = useState(false);
  const held = hovered || focused;
  useEffect(() => {
    if (!shown || held) return;
    const timer = setTimeout(() => {
      if (reduce) close();
      else setFading(true);
    }, NOTE_POPUP_TIMING.showMs);
    return () => {
      clearTimeout(timer);
      setFading(false);
    };
  }, [shown, held, reduce, close]);
  useEffect(() => {
    if (!fading) return;
    const timer = setTimeout(close, NOTE_POPUP_TIMING.fadeMs);
    return () => {
      clearTimeout(timer);
    };
  }, [fading, close]);
  return {
    fading,
    hold: {
      onPointerEnter: () => {
        setHovered(true);
      },
      onPointerLeave: () => {
        setHovered(false);
      },
      onFocus: () => {
        setFocused(true);
      },
      onBlur: (e: FocusEvent) => {
        if (!e.currentTarget.contains(e.relatedTarget)) setFocused(false);
      },
    },
  };
}

/** Fits this pop-up window's height to `content` as it renders (fonts scale; text wraps). */
function fitWindow(content: HTMLElement | null) {
  if (content === null || !isTauri()) return;
  const observer = new ResizeObserver(() => {
    const height = Math.ceil(content.getBoundingClientRect().height);
    void import('@tauri-apps/api/window')
      .then(({ getCurrentWindow, LogicalSize }) =>
        getCurrentWindow().setSize(new LogicalSize(window.innerWidth, height)),
      )
      .catch(() => undefined);
  });
  observer.observe(content);
  return () => {
    observer.disconnect();
  };
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
  // One action at a time: a second click must not reply twice.
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const close = useCallback(() => {
    setClosed(true);
    void closeWindow().catch(() => undefined);
  }, []);
  const { device, key } = target;
  const [phone, setPhone] = useState(target.name);
  useEffect(() => {
    port.phoneNotes(device).then(setView, close);
    port.linkedDevices().then(
      (devices) => {
        const linked = devices.find((d) => d.id === device);
        if (linked !== undefined) setPhone(linked.name);
      },
      () => undefined,
    );
  }, [port, device, close]);
  const model = view === null ? null : popupModel(view, key, phone);
  // Gone from the phone before it showed: nothing to pop up.
  const gone = view !== null && model === null;
  useEffect(() => {
    if (gone) void closeWindow().catch(() => undefined);
  }, [gone]);
  const { fading, hold } = useAutoDismiss(model !== null, close);
  if (closed || model === null) return null;
  const act = (action: NoteAction) => {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setProblem(null);
    port
      .phoneNoteAction(device, action)
      .then(
        (outcome) => {
          if (outcome === 'done') close();
          else setProblem(outcome);
        },
        () => {
          setProblem('failed');
        },
      )
      .finally(() => {
        pending.current = false;
        setBusy(false);
      });
  };
  return (
    <div
      ref={fitWindow}
      className={cx('s-note-window', fading && 'is-out')}
      style={{ transitionDuration: `${String(NOTE_POPUP_TIMING.fadeMs)}ms` }}
      {...hold}
    >
      <NotePopup
        model={model}
        problem={problem}
        busy={busy}
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
    </div>
  );
}
