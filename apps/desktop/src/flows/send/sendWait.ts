import type { SessionProgress } from '../../app/state/session';
import type { ErrorCode, RequestSummary } from '../../engine/types';
import type { SendWaitModel, WaitState } from '../../screens/send/model';
import { REQUEST_SECS } from '../../screens/send/countdown';
import type { MoveFacts } from '../../screens/send/model';
import type { SendFlowState } from './sendFlow';
import { sendMove } from './sendProgressViews';

/** How long a request waits for an answer, in ms. */
export const REQUEST_MS = REQUEST_SECS * 1000;

/** How each way a request can end reads to the sender; any other failure reads as "can't reach". */
const ENDINGS: Readonly<Partial<Record<ErrorCode, WaitState>>> = {
  request_declined: 'declined',
  request_expired: 'timeout',
  // Forgotten there or requests off: it doesn't say which (ADR 0004).
  request_refused: 'needs_code',
  network: 'away',
};

/** The counts a request carries, from the rail's facts. */
export const requestSummary = (facts: MoveFacts | null): RequestSummary | null =>
  facts === null
    ? null
    : {
        apps: facts.apps,
        bytes: facts.bytes,
        has_sensitive: facts.keys + (facts.secrets ?? 0) > 0,
      };

/**
 * The waiting dialog of a requested send: waiting until the receiver
 * connects, then reviewing; else how it ended. `null` without a request,
 * once the move is done, or when the user cancelled it.
 */
export function sendWaitModel(
  state: SendFlowState,
  progress: SessionProgress,
): SendWaitModel | null {
  const { request, run } = state;
  if (request === null) return null;
  const model = (wait: WaitState): SendWaitModel => ({
    state: wait,
    peer: request.name,
    expiresAt: request.at + REQUEST_MS,
    summary: requestSummary(sendMove(state, null).facts),
  });
  switch (run.status) {
    case 'loading':
      return model(progress.link === 'none' ? 'waiting' : 'reviewing');
    case 'failed':
      return run.error.code === 'cancelled' ? null : model(ENDINGS[run.error.code] ?? 'away');
    case 'idle':
    case 'ready':
      return null;
  }
}
