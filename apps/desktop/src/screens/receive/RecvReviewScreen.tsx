import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  ActionBar,
  Banner,
  Button,
  FlowLayout,
  Head,
  Icon,
  LinkRow,
  List,
  Stat,
  Stats,
} from '../../ui';
import { ReviewDecisions, unanswered } from './ReviewDecisions';
import { SpaceRow } from './SpaceRow';
import type { BlockerModel, ReviewIntents, ReviewModel } from './model';

export interface RecvReviewScreenProps {
  readonly model: ReviewModel;
  readonly on: ReviewIntents;
}

/**
 * The approval gate: facts first (three numbers), then decisions. Approve
 * unlocks when each has an answer; per-file rules are one tap away.
 */
export function RecvReviewScreen({ model, on }: RecvReviewScreenProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const blockerText = (b: BlockerModel): string => {
    switch (b.kind) {
      case 'unconfirmed_sensitive':
        return t('receive.review.blocker.unconfirmed_sensitive', { what: b.what, why: b.why });
      case 'running_app':
        return t('receive.review.blocker.running_app', { app: b.app });
      case 'free_space':
        return t('receive.review.blocker.free_space', {
          needed: format.bytes(b.needed),
          free: format.bytes(b.free),
        });
    }
  };
  const left = unanswered(model.decisions);
  const blocked = left > 0 || model.blockers.length > 0 || model.busy;
  return (
    <FlowLayout
      actions={
        <ActionBar
          start={<Button onClick={on.onCancel}>{t('receive.review.cancel')}</Button>}
          note={left > 0 ? t('receive.review.needs', { count: left }) : ''}
          end={
            <Button kind="primary" disabled={blocked} onClick={on.onApprove}>
              {t('receive.review.approve')}
            </Button>
          }
        />
      }
    >
      <Head
        title={t('receive.review.title')}
        lede={
          model.undoDays === null
            ? t('receive.review.ledeKept')
            : t('receive.review.lede', { days: model.undoDays })
        }
      />
      <Stats>
        <Stat
          value={format.number(model.counts.added)}
          label={t('receive.review.added', { count: model.counts.added })}
        />
        <Stat value={format.number(model.counts.replaced)} label={t('receive.review.replaced')} />
        <Stat value={format.number(model.counts.identical)} label={t('receive.review.identical')} />
      </Stats>
      {model.decisions.length > 0 && <ReviewDecisions decisions={model.decisions} on={on} />}
      {model.blockers.map((blocker, i) => (
        <Banner
          key={`${blocker.kind}-${String(i)}`}
          tone="warning"
          title={t('receive.review.blocked')}
          body={blockerText(blocker)}
        />
      ))}
      <List>
        <SpaceRow space={model.space} onIgnore={on.onIgnoreSpace} />
        <LinkRow
          lead={<Icon name="swap" />}
          title={t('receive.review.every')}
          sub={t('receive.review.everySub')}
          trail={<Icon name="chev" />}
          onClick={on.onSeeEvery}
        />
      </List>
    </FlowLayout>
  );
}
