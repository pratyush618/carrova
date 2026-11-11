import type { TFunction } from 'i18next';
import type { Notice, NoticeKind } from '../../engine/types';
import { calendarDaysBetween, isNearDay } from '../../i18n/format';
import type { Formatters } from '../../i18n/useFormat';
import type { BellGroup, BellRow } from '../../ui';

/** How each kind of notice looks in the bell. */
const LOOK: Record<NoticeKind, Pick<BellRow, 'icon' | 'tone'>> = {
  send_request: { icon: 'receive', tone: 'accent' },
  move_finished: { icon: 'check', tone: 'success' },
  move_failed: { icon: 'warn', tone: 'warning' },
  device_nearby: { icon: 'radar', tone: 'neutral' },
  update_ready: { icon: 'refresh', tone: 'neutral' },
};

/** What the bell lists: the notices by day, and how many are new. */
export interface BellContent {
  readonly unread: number;
  readonly groups: readonly BellGroup[];
}

/** "14:02" today, "Yesterday, 14:02" a day back, else the date. */
function whenText(date: Date, now: Date, t: TFunction, format: Formatters): string {
  if (calendarDaysBetween(date, now) === 0) return format.time(date);
  return isNearDay(date, now)
    ? t('summary.dayTime', { day: format.day(date, now, true), time: format.time(date) })
    : format.date(date);
}

/** The bell's content for `notices` (newest first) at `now`; empty groups are left out. */
export function bellContent(
  notices: readonly Notice[],
  now: Date,
  t: TFunction,
  format: Formatters,
): BellContent {
  const row = (notice: Notice, date: Date): BellRow => ({
    id: notice.id,
    ...LOOK[notice.kind],
    peer: notice.peer,
    summary: notice.summary,
    when: whenText(date, now, t, format),
    unread: !notice.read,
    review: notice.kind === 'send_request',
  });
  const today: BellRow[] = [];
  const earlier: BellRow[] = [];
  for (const notice of notices) {
    const date = new Date(notice.at * 1000);
    (calendarDaysBetween(date, now) === 0 ? today : earlier).push(row(notice, date));
  }
  return {
    unread: notices.filter((n) => !n.read).length,
    groups: [
      { title: t('bell.today'), rows: today },
      { title: t('bell.earlier'), rows: earlier },
    ].filter((group) => group.rows.length > 0),
  };
}
