import { useCallback, useEffect, useReducer, useRef } from 'react';
import { toBridgeError } from '../engine/errors';
import { idle, remoteReducer, type Remote } from './remote';

export interface Command<A extends readonly unknown[], T> {
  readonly state: Remote<T>;
  /** Runs the call; resolves with its value, or `null` if it failed or was overtaken. */
  readonly run: (...args: A) => Promise<T | null>;
  readonly reset: () => void;
}

/**
 * Runs a port call on demand and tracks it as a {@link Remote}. A newer run
 * overtakes an older one: only the latest result lands.
 */
export function useCommand<A extends readonly unknown[], T>(
  call: (...args: A) => Promise<T>,
): Command<A, T> {
  const [state, dispatch] = useReducer(remoteReducer<T>, idle);
  const latest = useRef(0);
  const run = useCallback(
    async (...args: A): Promise<T | null> => {
      const ticket = ++latest.current;
      dispatch({ type: 'start' });
      try {
        const value = await call(...args);
        if (ticket !== latest.current) return null;
        dispatch({ type: 'ok', value });
        return value;
      } catch (reason) {
        if (ticket === latest.current) dispatch({ type: 'fail', error: toBridgeError(reason) });
        return null;
      }
    },
    [call],
  );
  const reset = useCallback(() => {
    latest.current++;
    dispatch({ type: 'reset' });
  }, []);
  return { state, run, reset };
}

/** A port query loaded on mount; `reload` asks again. */
export function useRemote<T>(load: () => Promise<T>): Command<[], T> {
  const command = useCommand(load);
  const { run } = command;
  useEffect(() => {
    void run();
  }, [run]);
  return command;
}
