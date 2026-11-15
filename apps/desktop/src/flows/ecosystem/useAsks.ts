import { useCallback, useState } from 'react';
import { useEngineDispatch, useLink } from '../../app/state/EngineState';
import type { PendingAsk } from '../../app/state/appState';
import { toBridgeError } from '../../engine/errors';
import type { CarrovaPort } from '../../engine/types';

/**
 * The oldest offer still waiting, and how to answer it (the answer closes it
 * at once). An answer that could not be delivered names its device in `failed`.
 */
export function useAsks(port: CarrovaPort): {
  readonly ask: PendingAsk | null;
  readonly answer: (accept: boolean) => void;
  /** The device an answer did not reach, until dismissed. */
  readonly failed: string | null;
  readonly dismissFailure: () => void;
} {
  const ask = useLink().asks[0] ?? null;
  const dispatch = useEngineDispatch();
  const [failed, setFailed] = useState<string | null>(null);
  const answer = useCallback(
    (accept: boolean) => {
      if (ask === null) return;
      dispatch({ type: 'ask_answered', id: ask.id });
      port.answerAsk(ask.id, accept).catch((reason: unknown) => {
        // `known_device`: it expired or its device left meanwhile; there is nothing to answer.
        if (toBridgeError(reason).code !== 'known_device') setFailed(ask.device);
      });
    },
    [ask, dispatch, port],
  );
  const dismissFailure = useCallback(() => {
    setFailed(null);
  }, []);
  return { ask, answer, failed, dismissFailure };
}
