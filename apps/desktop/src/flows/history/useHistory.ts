import { useCallback, useMemo } from 'react';
import { useEngineDispatch } from '../../app/state/EngineState';
import type { CarrovaPort, SessionId, SessionRecord } from '../../engine/types';
import { current, type Remote } from '../remote';
import { useRemote } from '../useRemote';

export interface HistoryController {
  readonly sessions: Remote<SessionRecord[]>;
  readonly records: readonly SessionRecord[];
  readonly reload: () => void;
  /** Forget a session, its staged data and its undo point. */
  readonly dismiss: (id: SessionId) => void;
}

export function useHistory(port: CarrovaPort): HistoryController {
  const engine = useEngineDispatch();
  const { state, run } = useRemote(useCallback(() => port.sessions(), [port]));
  const dismiss = useCallback(
    (id: SessionId) => {
      void port.dismiss(id).then(() => {
        engine({ type: 'dismissed', session: id });
        void run();
      });
    },
    [port, engine, run],
  );
  const reload = useCallback(() => void run(), [run]);
  return useMemo(
    () => ({ sessions: state, records: current(state) ?? [], reload, dismiss }),
    [state, reload, dismiss],
  );
}
