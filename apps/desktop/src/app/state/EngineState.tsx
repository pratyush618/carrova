import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useReducer,
  useState,
  type ReactNode,
} from 'react';
import { usePort } from '../../engine/port';
import type { PairInviteView, PairPeerView, SessionId } from '../../engine/types';
import {
  appReducer,
  bridgeAction,
  emptySession,
  initialAppState,
  type AppAction,
  type AppState,
  type LinkState,
} from './appState';
import type { SessionProgress } from './session';

interface EngineStateApi {
  readonly state: AppState;
  readonly dispatch: (action: AppAction) => void;
  /** How many `send_requested` events arrived (each: paths wait in the bridge). */
  readonly sendRequests: number;
  /** How many `notice` events arrived (each: the inbox has something new). */
  readonly noticeEvents: number;
  /** The device a move last remembered (`null` until one is). */
  readonly remembered: Remembered | null;
  /** How many `show_nearby` events arrived (each: the tray asked for the Nearby page). */
  readonly nearbyRequests: number;
  /** The last pairing invite, and when it arrived (ms since epoch); kept only while its dialog needs it. */
  readonly invite: Invite | null;
  /** The device that last answered an invite, waiting for the key check; kept like the invite. */
  readonly found: Found | null;
  /** Drop the invite and the device found (their dialog closed, or the code ran out): the code leaves memory. */
  readonly clearInvite: () => void;
}

/** A pairing invite as it arrived. */
export interface Invite {
  readonly view: PairInviteView;
  readonly receivedAt: number;
}

/** A device found while pairing (`pair_peer_found`), as it arrived. */
export interface Found {
  readonly view: PairPeerView;
  readonly receivedAt: number;
}

/** A device a move just remembered, as `device_remembered` names it. */
export interface Remembered {
  readonly name: string;
  readonly fingerprint: string;
}

const EngineStateContext = createContext<EngineStateApi | null>(null);

/** Folds the port's event stream into {@link AppState}; the only subscriber in the app. */
export function EngineStateProvider({ children }: { readonly children: ReactNode }) {
  const port = usePort();
  const [state, dispatch] = useReducer(appReducer, initialAppState);
  const [sendRequests, setSendRequests] = useState(0);
  const [noticeEvents, setNoticeEvents] = useState(0);
  const [remembered, setRemembered] = useState<Remembered | null>(null);
  const [nearbyRequests, setNearbyRequests] = useState(0);
  const [invite, setInvite] = useState<Invite | null>(null);
  const [found, setFound] = useState<Found | null>(null);
  const clearInvite = useCallback(() => {
    setInvite(null);
    setFound(null);
  }, []);
  useEffect(() => {
    let stop: (() => void) | null = null;
    let live = true;
    void port
      .subscribe((event) => {
        // Forwarded sends and notices are not session state: the shell takes the paths, the bell reloads.
        if (event.kind === 'send_requested') setSendRequests((n) => n + 1);
        else if (event.kind === 'notice') setNoticeEvents((n) => n + 1);
        else if (event.kind === 'show_nearby') setNearbyRequests((n) => n + 1);
        // A pairing invite belongs to its dialog, not to a session.
        else if (event.kind === 'pair_invite')
          setInvite({ view: event.invite, receivedAt: Date.now() });
        else if (event.kind === 'pair_peer_found')
          setFound({ view: event.peer, receivedAt: Date.now() });
        else {
          if (event.kind === 'engine' && event.event.event === 'device_remembered') {
            const { name, fingerprint } = event.event;
            setRemembered({ name, fingerprint });
          }
          dispatch(bridgeAction(event, Date.now()));
        }
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
  const api = useMemo(
    () => ({
      state,
      dispatch,
      sendRequests,
      noticeEvents,
      remembered,
      nearbyRequests,
      invite,
      found,
      clearInvite,
    }),
    [state, sendRequests, noticeEvents, remembered, nearbyRequests, invite, found, clearInvite],
  );
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

/** Changes on each `notice` event (something new reached the inbox). */
export function useNoticeEvents(): number {
  return useEngineState().noticeEvents;
}

/** The device a move last remembered; a new object each time one is. */
export function useRemembered(): Remembered | null {
  return useEngineState().remembered;
}

/** Changes on each `show_nearby` event (the tray's "Nearby"). */
export function useNearbyRequests(): number {
  return useEngineState().nearbyRequests;
}

/** What linked devices did (asks, transfers, skipped clips, change counters). */
export function useLink(): LinkState {
  return useEngineState().state.link;
}

/** The last pairing invite and device found (`null` until one arrives, and once cleared), and how to drop them. */
export function useInvite(): {
  readonly invite: Invite | null;
  readonly found: Found | null;
  readonly clear: () => void;
} {
  const { invite, found, clearInvite } = useEngineState();
  return { invite, found, clear: clearInvite };
}
