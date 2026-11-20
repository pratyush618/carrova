import { useCallback, useMemo } from 'react';
import { useEngineDispatch, useRequests } from '../../app/state/EngineState';
import type { CarrovaPort, MoveRequestView, SessionId } from '../../engine/types';

/** Move requests from trusted computers waiting here; screens answer them through this. */
export interface MoveRequestsController {
  /** Oldest first, one per computer. */
  readonly pending: readonly MoveRequestView[];
  /** The computer whose request expired unanswered, until dismissed. */
  readonly expired: string | null;
  /** Open Review: resolves with its import session, or `null` if the request ended meanwhile. */
  readonly review: (id: number) => Promise<SessionId | null>;
  readonly decline: (id: number) => void;
  readonly dismissExpired: () => void;
}

/** An answer closes its request at once; one that comes too late changes nothing there. */
export function useMoveRequests(port: CarrovaPort): MoveRequestsController {
  const { pending, expired } = useRequests();
  const dispatch = useEngineDispatch();
  const review = useCallback(
    (id: number) => {
      dispatch({ type: 'request_answered', id });
      return port.answerRequest(id, 'review').then(
        (opened) => opened?.session ?? null,
        () => null,
      );
    },
    [dispatch, port],
  );
  const decline = useCallback(
    (id: number) => {
      dispatch({ type: 'request_answered', id });
      // Declining one that already ended is the same outcome: nothing moves.
      port.answerRequest(id, 'decline').catch(() => undefined);
    },
    [dispatch, port],
  );
  const dismissExpired = useCallback(() => {
    dispatch({ type: 'expired_dismissed' });
  }, [dispatch]);
  return useMemo(
    () => ({ pending, expired, review, decline, dismissExpired }),
    [pending, expired, review, decline, dismissExpired],
  );
}
