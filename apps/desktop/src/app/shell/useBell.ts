import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { bellContent } from '../../flows/inbox/inboxViews';
import { useFormat } from '../../i18n/useFormat';
import type { BellModel } from '../../ui';
import { useControllers } from '../controllers/Controllers';
import type { NavEvent } from '../nav/navMachine';

/** The header bell: the inbox's notices, and what its actions do (Review a request, open Settings). */
export function useBell(go: (event: NavEvent) => void): BellModel {
  const { t } = useTranslation();
  const format = useFormat();
  const { inbox, receive } = useControllers();
  const { notices, markRead } = inbox;
  return useMemo(
    () => ({
      ...bellContent(notices, new Date(), t, format),
      onReview: (id) => {
        markRead([id]);
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
    [notices, markRead, receive, go, t, format],
  );
}
