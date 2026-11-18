import { useCallback, useEffect, useRef, useState } from 'react';
import type { CallLogView } from '../../bindings/CallLogView';
import type { CarrovaPort } from '../../engine/types';

/** The user's clock for "Today" and "Yesterday". */
const clock = () => ({ now_ms: Date.now(), utc_offset_min: -new Date().getTimezoneOffset() });

/** `older` after `log`: a day split across the pages becomes one again. */
export function appendPage(log: CallLogView, older: CallLogView): CallLogView {
  const last = log.groups.at(-1);
  const [first, ...rest] = older.groups;
  if (last === undefined || first?.days_ago !== last.days_ago)
    return { groups: [...log.groups, ...older.groups], next: older.next };
  const joined = { ...last, rows: [...last.rows, ...first.rows] };
  return { groups: [...log.groups.slice(0, -1), joined, ...rest], next: older.next };
}

/**
 * Phone `id`'s call log from the phone (not kept here), newest page first,
 * `null` until it answers; `failed` when it couldn't be read. Nothing is
 * asked while `on` is false (calls are off for it).
 */
export function useCallLog(
  port: CarrovaPort,
  id: string,
  on: boolean,
): {
  readonly log: CallLogView | null;
  readonly failed: boolean;
  readonly more: () => void;
} {
  const [log, setLog] = useState<CallLogView | null>(null);
  const [failed, setFailed] = useState(false);
  // The first page's clock, for every older page: a day stays one group across midnight.
  const at = useRef(clock());
  // Set at once (not on the next render), so a second click finds it.
  const asking = useRef(false);
  // Which log (phone, calls on) is shown: bumped each time one ends.
  const run = useRef(0);
  useEffect(() => {
    if (!on) return;
    let live = true;
    at.current = clock();
    port.callHistory(id, null, at.current).then(
      (page) => {
        if (live) setLog(page);
      },
      () => {
        if (live) setFailed(true);
      },
    );
    return () => {
      live = false;
      // An older page still on its way belongs to the log that ends here.
      run.current += 1;
      asking.current = false;
    };
  }, [port, id, on]);
  const more = useCallback(() => {
    if (!on || log?.next == null || asking.current) return;
    asking.current = true;
    const mine = run.current;
    port.callHistory(id, log.next, at.current).then(
      (page) => {
        if (run.current !== mine) return;
        asking.current = false;
        setLog((now) => (now === null ? page : appendPage(now, page)));
      },
      () => {
        if (run.current !== mine) return;
        asking.current = false;
        setFailed(true);
      },
    );
  }, [port, id, on, log]);
  return { log, failed, more };
}
