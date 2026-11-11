import {
  createContext,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useState,
  type ReactNode,
} from 'react';
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
  /** How many `send_requested` events arrived (each: paths wait in the bridge). */
  readonly sendRequests: number;
}

const EngineStateContext = createContext<EngineStateApi | null>(null);

/** Folds the port's event stream into {@link AppState}; the only subscriber in the app. */
export function EngineStateProvider({ children }: { readonly children: ReactNode }) {
  const port = usePort();
  const [state, dispatch] = useReducer(appReducer, initialAppState);
  const [sendRequests, setSendRequests] = useState(0);
  useEffect(() => {
    let stop: (() => void) | null = null;
    let live = true;
    void port
      .subscribe((event) => {
        // Forwarded sends are not session state: the shell takes their paths.
        if (event.kind === 'send_requested') setSendRequests((n) => n + 1);
        else dispatch(bridgeAction(event, Date.now()));
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
  const api = useMemo(() => ({ state, dispatch, sendRequests }), [state, sendRequests]);
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

/** Changes on each `send_requested` event (paths sent from the file manager). */
export function useSendRequests(): number {
  return useEngineState().sendRequests;
}

/** Raise an app action the UI owns (prompt answered, session dismissed). */
export function useEngineDispatch(): (action: AppAction) => void {
  return useEngineState().dispatch;
}
