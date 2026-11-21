import { useCallback, useMemo } from 'react';
import { useEngineDispatch, useRequests } from '../../app/state/EngineState';
import type { EndedRequest } from '../../app/state/requests';
import type { CarrovaPort, MoveRequestView, SessionId } from '../../engine/types';

/** Move requests from trusted computers waiting here; screens answer them through this. */
export interface MoveRequestsController {
  /** Oldest first, one per computer. */
  readonly pending: readonly MoveRequestView[];
  /** The last request that ended unanswered, until dismissed. */
  readonly ended: EndedRequest | null;
  /** Open Review: resolves with its import session, or `null` (and a notice) if the request ended meanwhile. */
  readonly review: (id: number) => Promise<SessionId | null>;
  readonly decline: (id: number) => void;
  readonly dismissEnded: () => void;
}

/** An answer closes its request at once; one that comes too late changes nothing there. */
export function useMoveRequests(port: CarrovaPort): MoveRequestsController {
  const { pending, ended } = useRequests();
  const dispatch = useEngineDispatch();
  const review = useCallback(
    (id: number) => {
      const name = pending.find((r) => r.id === id)?.name;
      dispatch({ type: 'request_answered', id });
      const missed = () => {
        if (name !== undefined) dispatch({ type: 'review_missed', name });
        return null;
      };
      return port.answerRequest(id, 'review').then((opened) => opened?.session ?? missed(), missed);
    },
    [dispatch, port, pending],
  );
  const decline = useCallback(
    (id: number) => {
      dispatch({ type: 'request_answered', id });
      // Declining one that already ended is the same outcome: nothing moves.
      port.answerRequest(id, 'decline').catch(() => undefined);
    },
    [dispatch, port],
  );
  const dismissEnded = useCallback(() => {
    dispatch({ type: 'ended_dismissed' });
  }, [dispatch]);
  return useMemo(
    () => ({ pending, ended, review, decline, dismissEnded }),
    [pending, ended, review, decline, dismissEnded],
  );
}
