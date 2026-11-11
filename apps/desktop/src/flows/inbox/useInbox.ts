import { useCallback, useEffect, useMemo } from 'react';
import { useNoticeEvents } from '../../app/state/EngineState';
import type { CarrovaPort, Notice, NoticeId } from '../../engine/types';
import { current } from '../remote';
import { useRemote } from '../useRemote';

export interface InboxController {
  /** The bell's notices, newest first (empty until loaded). */
  readonly notices: readonly Notice[];
  /** Mark `ids` (or everything) read. */
  readonly markRead: (ids: NoticeId[] | 'all') => void;
}

/** The bell's notices: loaded on mount and again whenever the engine reports a new one. */
export function useInbox(port: CarrovaPort): InboxController {
  const events = useNoticeEvents();
  const { state, run } = useRemote(useCallback(() => port.notices(), [port]));
  useEffect(() => {
    if (events > 0) void run();
  }, [events, run]);
  const markRead = useCallback(
    (ids: NoticeId[] | 'all') => {
      void port.markNoticesRead(ids).then(() => run());
    },
    [port, run],
  );
  return useMemo(() => ({ notices: current(state) ?? [], markRead }), [state, markRead]);
}
