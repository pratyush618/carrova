import type { MoveRequestView, RequestEnd } from '../../engine/types';

/** Move requests waiting at this computer, and the last one that ran out unanswered. */
export interface RequestsState {
  /** Oldest first; one per computer (a newer request replaces its older one). */
  readonly pending: readonly MoveRequestView[];
  /** The computer whose request expired, until the notice is dismissed. */
  readonly expired: string | null;
}

export const noRequests: RequestsState = { pending: [], expired: null };

export type RequestsAction =
  | { readonly type: 'request_arrived'; readonly request: MoveRequestView }
  | { readonly type: 'request_ended'; readonly id: number; readonly why: RequestEnd }
  /** The user answered it: it leaves at once. */
  | { readonly type: 'request_answered'; readonly id: number }
  | { readonly type: 'expired_dismissed' };

const without = (pending: readonly MoveRequestView[], id: number) =>
  pending.filter((r) => r.id !== id);

/** The requests reducer. Only an expiry is worth a notice; any other ending is silent. */
export function applyRequests(state: RequestsState, action: RequestsAction): RequestsState {
  switch (action.type) {
    case 'request_arrived': {
      const { request } = action;
      const others = state.pending.filter((r) => r.device !== request.device);
      return { ...state, pending: [...others, request] };
    }
    case 'request_ended': {
      const ended = state.pending.find((r) => r.id === action.id);
      return {
        pending: without(state.pending, action.id),
        expired: action.why === 'expired' && ended ? ended.name : state.expired,
      };
    }
    case 'request_answered':
      return { ...state, pending: without(state.pending, action.id) };
    case 'expired_dismissed':
      return { ...state, expired: null };
  }
}
