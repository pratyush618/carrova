import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { LocalClock } from '../../../bindings/LocalClock';
import type { ThreadView } from '../../../bindings/ThreadView';
import type { ThreadsView } from '../../../bindings/ThreadsView';
import { usePort } from '../../../engine/port';
import type { LinkedDeviceView } from '../../../engine/types';
import { messagesPhone } from '../../../flows/phone/messagesPhone';
import { loadPages } from '../../../flows/phone/pages';
import { usePhoneSims } from '../../../flows/phone/usePhoneSims';
import { useSmsSends, type SmsSend } from '../../../flows/phone/useSmsSends';
import { MessagesScreen } from '../../../screens/phone/messages/MessagesScreen';
import { HubLayout } from '../../../ui';
import { useControllers } from '../../controllers/Controllers';
import { useLink } from '../../state/EngineState';

/** Now on this computer's clock and time zone: the engine counts days on it. */
const localClock = (): LocalClock => ({
  now_ms: Date.now(),
  utc_offset_min: -new Date().getTimezoneOffset(),
});

/** `older` below `newer`, without a conversation that moved up between the two loads. */
const joinThreads = (newer: ThreadsView, older: ThreadsView): ThreadsView => ({
  threads: [
    ...newer.threads,
    ...older.threads.filter((o) => !newer.threads.some((n) => n.id === o.id)),
  ],
  next: older.next,
});

/** `older` above `newer` (chat order: oldest first). */
const joinMessages = (newer: ThreadView, older: ThreadView): ThreadView => ({
  messages: [...older.messages, ...newer.messages],
  next: older.next,
});

/** Messages: the texts of the linked phone that shares them (empty until one does). */
export function MessagesRoute() {
  const { linked } = useControllers();
  const phone = messagesPhone(linked.devices);
  if (phone === undefined) return <HubLayout>{null}</HubLayout>;
  return <PhoneMessages key={phone.id} phone={phone} />;
}

/**
 * One phone's conversations. Loads (again) while it is linked, whenever its
 * texts change; while it is away what was loaded stays and sending waits.
 * Received texts the phone has not stored yet join the list on top.
 */
function PhoneMessages({ phone }: { readonly phone: LinkedDeviceView }) {
  const port = usePort();
  const { sms, texts } = useLink();
  const { id } = phone;
  const away = phone.state !== 'linked';
  const incoming = useMemo(
    () => sms.filter((p) => p.device === id).map((p) => p.message),
    [sms, id],
  );
  const [list, setList] = useState<ThreadsView | null>(null);
  const threads = list?.threads ?? null;
  // Pages of conversations shown: the user asks for older ones.
  const [listPages, setListPages] = useState(1);
  const [thread, setThread] = useState<ThreadView | null>(null);
  const [failed, setFailed] = useState(false);
  const [picked, setPicked] = useState<number | null>(null);
  const [composing, setComposing] = useState(false);
  // Bumped once a sent text is out: the list shows it.
  const [sent, setSent] = useState(0);
  const open = composing ? null : (picked ?? threads?.[0]?.id ?? null);
  // Pages of the open conversation shown: the user asks for older texts.
  const [older, setOlder] = useState<{ thread: number | null; pages: number }>({
    thread: null,
    pages: 1,
  });
  const pages = older.thread === open ? older.pages : 1;
  // Read when a send ends: the user may have moved on since.
  const now = useRef({ composing, open, pages });
  useEffect(() => {
    now.current = { composing, open, pages };
  });
  const loadThread = useCallback(
    (thread: number, count: number) =>
      loadPages((before) => port.phoneThread(id, thread, before), count, joinMessages),
    [port, id],
  );
  // Each load of the open conversation takes a ticket: only the newest shows.
  const latest = useRef(0);

  useEffect(() => {
    if (away) return;
    let live = true;
    loadPages(
      (before) => port.phoneThreads(id, before, incoming, localClock()),
      listPages,
      joinThreads,
    ).then(
      (view) => {
        if (!live) return;
        setList(view);
        setFailed(false);
      },
      () => {
        if (live) setFailed(true);
      },
    );
    return () => {
      live = false;
    };
  }, [port, id, away, incoming, texts, sent, listPages]);

  useEffect(() => {
    if (away || open === null) return;
    let live = true;
    const ticket = ++latest.current;
    loadThread(open, pages).then(
      (view) => {
        if (live && ticket === latest.current) setThread(view);
      },
      () => {
        if (live && ticket === latest.current) setFailed(true);
      },
    );
    return () => {
      live = false;
    };
  }, [loadThread, away, open, pages, texts]);

  // A sent text stays shown until its conversation, stored copy included, is here.
  const onSent = useCallback(
    async (s: SmsSend) => {
      setSent((n) => n + 1);
      if (s.thread === null) {
        // A new message's conversation is the newest once the list reloads.
        if (now.current.composing) {
          setComposing(false);
          setPicked(null);
        }
        return;
      }
      const shown = now.current.open === s.thread;
      // A hidden conversation loads itself when opened again, after this.
      const ticket = shown ? ++latest.current : latest.current;
      try {
        const view = await loadThread(s.thread, shown ? now.current.pages : 1);
        if (ticket === latest.current && now.current.open === s.thread) setThread(view);
      } catch {
        setFailed(true);
      }
    },
    [loadThread],
  );
  const { sends, send, retry } = useSmsSends(port, id, onSent);
  const sims = usePhoneSims(port, id, 'messages', !away);

  return (
    <MessagesScreen
      model={{
        phone: phone.name,
        away,
        failed,
        threads,
        moreThreads: (list?.next ?? null) !== null,
        selected: threads?.find((row) => row.id === open) ?? null,
        thread,
        sends: sends.filter((s) => s.thread === open),
        composing,
        sims,
      }}
      on={{
        onSelect: (thread) => {
          setComposing(false);
          setThread(null);
          setPicked(thread);
        },
        onCompose: () => {
          setComposing(true);
          setThread(null);
        },
        onSend: (to, text, sim) => {
          send(to, text, open, sim);
        },
        onRetry: retry,
        onOlderMessages: () => {
          setOlder({ thread: open, pages: pages + 1 });
        },
        onMoreThreads: () => {
          setListPages((n) => n + 1);
        },
      }}
    />
  );
}
