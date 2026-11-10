import { createContext, useContext, useEffect, useMemo, useReducer, type ReactNode } from 'react';
import { usePort } from '../../engine/port';
import type { SessionId } from '../../engine/types';
import {
  appReducer,
  bridgeAction,
  emptySession,
  initialAppState,
  type AppAction,
  type AppState,
} from './appState';
import type { SessionProgress } from './session';

interface EngineStateApi {
  readonly state: AppState;
  readonly dispatch: (action: AppAction) => void;
}

const EngineStateContext = createContext<EngineStateApi | null>(null);

/** Folds the port's event stream into {@link AppState}; the only subscriber in the app. */
export function EngineStateProvider({ children }: { readonly children: ReactNode }) {
  const port = usePort();
  const [state, dispatch] = useReducer(appReducer, initialAppState);
  useEffect(() => {
    let stop: (() => void) | null = null;
    let live = true;
    void port
      .subscribe((event) => {
        dispatch(bridgeAction(event));
      })
      .then((unsubscribe) => {
        if (live) stop = unsubscribe;
        else unsubscribe();
      });
    return () => {
      live = false;
      stop?.();
    };
  }, [port]);
  const api = useMemo(() => ({ state, dispatch }), [state]);
  return <EngineStateContext value={api}>{children}</EngineStateContext>;
}

function useEngineState(): EngineStateApi {
  const api = useContext(EngineStateContext);
  if (api === null) throw new Error('useEngineState must be used inside EngineStateProvider');
  return api;
}

/** Progress of one session (an empty one until its first event, or for `null`). */
export function useSession(id: SessionId | null): SessionProgress {
  const { state } = useEngineState();
  return (id === null ? undefined : state.sessions[id]) ?? emptySession(id ?? '');
}

/** Raise an app action the UI owns (prompt answered, session dismissed). */
export function useEngineDispatch(): (action: AppAction) => void {
  return useEngineState().dispatch;
}
