import { useCallback } from 'react';
import { EndedRequestNote, MoveRequestCard } from '../../../screens/home/MoveRequestCard';
import { useControllers } from '../../controllers/Controllers';
import type { NavEvent } from '../../nav/navMachine';
import { useNav } from '../nav';

/**
 * Open Review of move request `id`: the normal receive review of the
 * session it opens. Takes `go`: the bell lives outside the navigation context.
 */
export function useReviewRequest(go: (event: NavEvent) => void): (id: number) => void {
  const { requests, receive } = useControllers();
  return useCallback(
    (id: number) => {
      void requests.review(id).then((session) => {
        if (session === null) return;
        receive.reviewRequested(session);
        go({ type: 'enterFlow', at: { flow: 'receive', step: 'review' } });
      });
    },
    [requests, receive, go],
  );
}

/** Home's move requests: each waiting one, and the last that ended unanswered. */
export function HomeMoveRequests() {
  const { requests, receive } = useControllers();
  const review = useReviewRequest(useNav().go);
  // One receive at a time: a request waits while another runs (it may still expire).
  const busy = receive.state.session !== null;
  return (
    <>
      {requests.pending.map((request) => (
        <MoveRequestCard
          key={request.id}
          request={request}
          busy={busy}
          on={{
            onReview: () => {
              review(request.id);
            },
            onDecline: () => {
              requests.decline(request.id);
            },
          }}
        />
      ))}
      {requests.ended !== null && (
        <EndedRequestNote ended={requests.ended} onDismiss={requests.dismissEnded} />
      )}
    </>
  );
}
