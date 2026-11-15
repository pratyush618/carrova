import { useCallback } from 'react';
import { useEngineDispatch, useLink } from '../../app/state/EngineState';
import type { PendingAsk } from '../../app/state/appState';
import type { CarrovaPort } from '../../engine/types';

/** The oldest offer still waiting, and how to answer it (the answer closes it at once). */
export function useAsks(port: CarrovaPort): {
  readonly ask: PendingAsk | null;
  readonly answer: (accept: boolean) => void;
} {
  const ask = useLink().asks[0] ?? null;
  const dispatch = useEngineDispatch();
  const answer = useCallback(
    (accept: boolean) => {
      if (ask === null) return;
      dispatch({ type: 'ask_answered', id: ask.id });
      // An ask that expired meanwhile is refused by the engine: nothing more to show.
      void port.answerAsk(ask.id, accept).catch(() => undefined);
    },
    [ask, dispatch, port],
  );
  return { ask, answer };
}
