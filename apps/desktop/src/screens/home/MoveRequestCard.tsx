import { useTranslation } from 'react-i18next';
import type { MoveRequestView } from '../../engine/types';
import { useFormat } from '../../i18n/useFormat';
import { Banner, Button } from '../../ui';
import { clock, useSecondsLeft } from '../send/countdown';
import { countsText } from '../shared/requestCounts';

/**
 * A trusted computer asks to send this one its setup (`sendreq:open`): who,
 * the counts and the time left, never app names. Review opens the normal
 * receive review; while another receive runs it waits (`busy`).
 */
export function MoveRequestCard({
  request,
  busy,
  on,
}: {
  readonly request: MoveRequestView;
  readonly busy: boolean;
  readonly on: { readonly onReview: () => void; readonly onDecline: () => void };
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const seconds = useSecondsLeft(request.expires_at * 1000);
  const sub = t('request.card.sub', {
    counts: countsText(request.summary, t, format),
    time: clock(seconds),
  });
  return (
    <Banner
      tone="accent"
      icon="receive"
      title={t('request.card.title', { name: request.name })}
      body={busy ? `${sub}. ${t('request.card.busy')}` : sub}
      action={
        <>
          <Button size="sm" kind="quiet" onClick={on.onDecline}>
            {t('request.card.decline')}
          </Button>
          <Button size="sm" kind="primary" disabled={busy} onClick={on.onReview}>
            {t('request.card.review')}
          </Button>
        </>
      }
    />
  );
}

/** A request ran out unanswered (`sendreq:expired`): nothing changed here. */
export function ExpiredRequestNote({
  name,
  onDismiss,
}: {
  readonly name: string;
  readonly onDismiss: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Banner
      tone="accent"
      icon="clock"
      title={t('request.expired.title', { name })}
      body={t('request.expired.body')}
      action={
        <Button size="sm" onClick={onDismiss}>
          {t('request.expired.dismiss')}
        </Button>
      }
    />
  );
}
