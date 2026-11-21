import type { MoveRequestView, RequestEnd } from '../../engine/types';

/** A request that ended unanswered: it ran out, or ended before its Review could open. */
export interface EndedRequest {
  /** The computer that asked. */
  readonly name: string;
  readonly why: 'expired' | 'missed';
}

/** Move requests waiting at this computer, and the last one that ended unanswered. */
export interface RequestsState {
  /** Oldest first; one per computer (a newer request replaces its older one). */
  readonly pending: readonly MoveRequestView[];
  /** The last request that ended unanswered, until the notice is dismissed. */
  readonly ended: EndedRequest | null;
}

export const noRequests: RequestsState = { pending: [], ended: null };

export type RequestsAction =
  | { readonly type: 'request_arrived'; readonly request: MoveRequestView }
  | { readonly type: 'request_ended'; readonly id: number; readonly why: RequestEnd }
  /** The user answered it: it leaves at once. */
  | { readonly type: 'request_answered'; readonly id: number }
  /** Review was asked for, but the request had ended meanwhile. */
  | { readonly type: 'review_missed'; readonly name: string }
  | { readonly type: 'ended_dismissed' };

const without = (pending: readonly MoveRequestView[], id: number) =>
  pending.filter((r) => r.id !== id);

/** The requests reducer. Only an expiry or a missed Review is worth a notice; any other ending is silent. */
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
        ended:
          action.why === 'expired' && ended ? { name: ended.name, why: 'expired' } : state.ended,
      };
    }
    case 'request_answered':
      return { ...state, pending: without(state.pending, action.id) };
    case 'review_missed':
      return { ...state, ended: { name: action.name, why: 'missed' } };
    case 'ended_dismissed':
      return { ...state, ended: null };
  }
}
