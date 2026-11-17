import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { LocalClock } from '../../../bindings/LocalClock';
import type { ThreadRow } from '../../../bindings/ThreadRow';
import type { ThreadView } from '../../../bindings/ThreadView';
import { usePort } from '../../../engine/port';
import type { LinkedDeviceView } from '../../../engine/types';
import { messagesPhone } from '../../../flows/phone/messagesPhone';
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
  const [threads, setThreads] = useState<readonly ThreadRow[] | null>(null);
  const [thread, setThread] = useState<ThreadView | null>(null);
  const [failed, setFailed] = useState(false);
  const [picked, setPicked] = useState<number | null>(null);
  const [composing, setComposing] = useState(false);
  // Bumped once a sent text is out: the list shows it.
  const [sent, setSent] = useState(0);
  const open = composing ? null : (picked ?? threads?.[0]?.id ?? null);
  // Read when a send ends: the user may have moved on since.
  const now = useRef({ composing, open });
  useEffect(() => {
    now.current = { composing, open };
  });

  useEffect(() => {
    if (away) return;
    let live = true;
    port.phoneThreads(id, null, incoming, localClock()).then(
      (view) => {
        if (!live) return;
        setThreads(view.threads);
        setFailed(false);
      },
      () => {
        if (live) setFailed(true);
      },
    );
    return () => {
      live = false;
    };
  }, [port, id, away, incoming, texts, sent]);

  useEffect(() => {
    if (away || open === null) return;
    let live = true;
    port.phoneThread(id, open, null).then(
      (view) => {
        if (live) setThread(view);
      },
      () => {
        if (live) setFailed(true);
      },
    );
    return () => {
      live = false;
    };
  }, [port, id, away, open, texts]);

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
      try {
        const view = await port.phoneThread(id, s.thread, null);
        if (now.current.open === s.thread) setThread(view);
      } catch {
        setFailed(true);
      }
    },
    [port, id],
  );
  const { sends, send, retry } = useSmsSends(port, id, onSent);

  return (
    <MessagesScreen
      model={{
        phone: phone.name,
        away,
        failed,
        threads,
        selected: threads?.find((row) => row.id === open) ?? null,
        thread,
        sends: sends.filter((s) => s.thread === open),
        composing,
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
        onSend: (to, text) => {
          send(to, text, open);
        },
        onRetry: retry,
      }}
    />
  );
}
