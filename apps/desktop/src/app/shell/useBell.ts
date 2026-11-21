import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { bellContent } from '../../flows/inbox/inboxViews';
import { useFormat } from '../../i18n/useFormat';
import type { BellModel } from '../../ui';
import { useControllers } from '../controllers/Controllers';
import type { NavEvent } from '../nav/navMachine';
import { useReviewRequest } from './routes/RequestRoutes';

/**
 * The header bell: the inbox's notices, and what its actions do (Review a
 * request, open Settings). Review opens the newest move request still
 * waiting; without one, a receive by code. Neither while a receive runs.
 */
export function useBell(go: (event: NavEvent) => void): BellModel {
  const { t } = useTranslation();
  const format = useFormat();
  const { inbox, receive, requests } = useControllers();
  const reviewRequest = useReviewRequest(go);
  const waiting = requests.pending.at(-1)?.id ?? null;
  const busy = receive.state.session !== null;
  const { notices, markRead } = inbox;
  return useMemo(
    () => ({
      ...bellContent(notices, new Date(), t, format),
      onReview: (id) => {
        markRead([id]);
        // One receive at a time: a running one stays, and a request keeps waiting on Home.
        if (busy) return;
        if (waiting !== null) {
          reviewRequest(waiting);
          return;
        }
        receive.start('lan');
        go({ type: 'startFlow', flow: 'receive' });
      },
      onMarkAllRead: () => {
        markRead('all');
      },
      onSettings: () => {
        go({ type: 'openHub', page: 'settings', section: 'notifications' });
      },
    }),
    [notices, markRead, receive, busy, waiting, reviewRequest, go, t, format],
  );
}
