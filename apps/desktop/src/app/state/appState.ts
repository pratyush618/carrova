import type {
  BridgeError,
  BridgeEvent,
  CodeView,
  EngineEvent,
  SessionId,
} from '../../engine/types';
import {
  applyLinkAction,
  applyLinkEvent,
  emptyLink,
  LINK_SESSION,
  type LinkAction,
  type LinkState,
} from './link';
import { applyEvent, emptySession, type NewCodePrompt, type SessionProgress } from './session';

export {
  LINK_SESSION,
  type ClipNote,
  type LinkState,
  type PendingAsk,
  type Transfer,
} from './link';

export { emptySession, applyEvent, type LinkStatus, type SessionProgress } from './session';

/** App-wide state. Engine-owned facts only arrive through {@link AppAction}s. */
export interface AppState {
  readonly sessions: Readonly<Record<SessionId, SessionProgress>>;
  /** Linked devices' events (the `link` session is no move). */
  readonly link: LinkState;
}

/** Everything that can change {@link AppState}. Bridge events arrive via {@link bridgeAction}. */
export type AppAction =
  | { readonly type: 'engine'; readonly session: SessionId; readonly event: EngineEvent }
  | {
      readonly type: 'pairing_code';
      readonly session: SessionId;
      readonly code: CodeView;
      /** When it arrived (ms since epoch): the code's lifetime counts from here. */
      readonly receivedAt: number;
    }
  | {
      readonly type: 'confirm_new_code';
      readonly session: SessionId;
      readonly prompt: NewCodePrompt;
    }
  /** The user answered the new-code question (the prompt closes). */
  | { readonly type: 'pairing_answered'; readonly session: SessionId }
  | { readonly type: 'task_ended'; readonly session: SessionId; readonly error: BridgeError | null }
  | { readonly type: 'dismissed'; readonly session: SessionId }
  | LinkAction;

export const initialAppState: AppState = { sessions: {}, link: emptyLink };

/** Bridge events about a session (everything but `send_requested`, `show_nearby`, `notice` and pairing's). */
export type SessionBridgeEvent = Exclude<
  BridgeEvent,
  { kind: 'send_requested' | 'show_nearby' | 'notice' | 'pair_invite' | 'pair_peer_found' }
>;

/** The app action for one session bridge event, received at `now` (ms since epoch). */
export function bridgeAction(event: SessionBridgeEvent, now: number): AppAction {
  switch (event.kind) {
    case 'engine':
      return { type: 'engine', session: event.session, event: event.event };
    case 'pairing_code':
      return { type: 'pairing_code', session: event.session, code: event.code, receivedAt: now };
    case 'confirm_new_code':
      return {
        type: 'confirm_new_code',
        session: event.session,
        prompt: { failures: event.failures, remaining: event.remaining },
      };
    case 'task_ended':
      return { type: 'task_ended', session: event.session, error: event.error };
  }
}

function fold(
  action: Exclude<AppAction, { type: 'dismissed' } | LinkAction>,
  s: SessionProgress,
): SessionProgress {
  switch (action.type) {
    case 'engine':
      return applyEvent(s, action.event);
    case 'pairing_code':
      return {
        ...s,
        code: {
          view: action.code,
          expiresAt: action.receivedAt + action.code.expires_in_secs * 1000,
        },
        checking: false,
        newCodePrompt: null,
      };
    case 'confirm_new_code':
      return { ...s, newCodePrompt: action.prompt };
    case 'pairing_answered':
      return { ...s, newCodePrompt: null };
    case 'task_ended':
      return { ...s, ended: { error: action.error }, newCodePrompt: null };
  }
}

/** The app reducer. */
export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'ask_answered':
    case 'transfer_dismissed':
    case 'clip_dismissed':
      return { ...state, link: applyLinkAction(state.link, action) };
    case 'dismissed': {
      const rest = Object.entries(state.sessions).filter(([id]) => id !== action.session);
      return { ...state, sessions: Object.fromEntries(rest) };
    }
    default: {
      if (action.type === 'engine' && action.session === LINK_SESSION)
        return { ...state, link: applyLinkEvent(state.link, action.event) };
      const current = state.sessions[action.session] ?? emptySession(action.session);
      return { ...state, sessions: { ...state.sessions, [action.session]: fold(action, current) } };
    }
  }
}
