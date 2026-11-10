import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  ActionBar,
  AppGlyph,
  Button,
  FlowLayout,
  GroupHead,
  Head,
  LinkButton,
  List,
  Row,
  Stat,
  Stats,
} from '../../ui';
import { MoveRail, RouteChip } from '../shared/MoveRail';
import { ReviewDecisions, unanswered } from './ReviewDecisions';
import { SpaceRow } from './SpaceRow';
import type { Retention } from '../../engine/types';
import type { ReviewIntents, ReviewModel } from './model';

export interface RecvReviewScreenProps {
  readonly model: ReviewModel;
  readonly on: ReviewIntents;
}

/** The retention as a copy key under `receive.review.undoKept`/`lede`, with its days to count. */
const retentionCopy = (retention: Retention) =>
  typeof retention === 'string'
    ? ({ key: retention, count: 0 } as const)
    : ({ key: 'keep_for', count: retention.keep_for.days } as const);

function ReviewRail({ model }: { readonly model: ReviewModel }) {
  const { t } = useTranslation();
  const format = useFormat();
  const { rail } = model;
  const keys =
    rail.keys === null
      ? t('receive.review.rail.keysUnanswered')
      : rail.keys.length === 0
        ? t('receive.review.rail.keysNone')
        : rail.keys.join(', ');
  return (
    <MoveRail
      route={rail.route}
      here="to"
      notes={{ from: t('receive.review.rail.source'), to: t('transfer.move.here') }}
      facts={[
        { label: t('receive.review.rail.apps'), value: t('receive.review.rail.appsOf', rail.apps) },
        { label: t('receive.review.rail.size'), value: format.bytes(rail.bytes) },
        { label: t('receive.review.rail.keys'), value: keys },
      ]}
      promise={t('receive.review.rail.promise')}
    />
  );
}

/** "Suggested, change anything": where things land, clashes and undo, each one tap from changing. */
function Suggestions({ model, on }: RecvReviewScreenProps) {
  const { t } = useTranslation();
  const os = (kind: NonNullable<ReviewModel['mapped']>['to']) => t(`settings.about.os.${kind}`);
  const { mapped, counts } = model;
  const kept = retentionCopy(model.retention);
  return (
    <section aria-label={t('receive.review.suggested')}>
      <GroupHead title={t('receive.review.suggested')} />
      <List>
        {mapped !== null && (
          <Row
            lead={<AppGlyph glyph={{ icon: 'swap' }} tone="accent" />}
            title={t('receive.review.mapped', { from: os(mapped.from), to: os(mapped.to) })}
            sub={t('receive.review.mappedSub', { count: mapped.apps, to: os(mapped.to) })}
            trail={<LinkButton onClick={on.onSeeWhere}>{t('receive.review.seeWhere')}</LinkButton>}
          />
        )}
        <Row
          lead={<AppGlyph glyph={{ icon: 'file' }} />}
          title={
            counts.replaced > 0
              ? t('receive.review.differ', { count: counts.replaced })
              : t('receive.review.noneDiffer')
          }
          sub={t(`receive.review.rule.${model.rule}`)}
          trail={<LinkButton onClick={on.onSeeEvery}>{t('receive.review.every')}</LinkButton>}
        />
        <Row
          lead={<AppGlyph glyph={{ icon: 'history' }} />}
          title={t(`receive.review.undoKept.${kept.key}`, { count: kept.count })}
          sub={t('receive.review.undoSub')}
          trail={
            <LinkButton aria-label={t('receive.review.changeUndo')} onClick={on.onSeeEvery}>
              {t('action.change')}
            </LinkButton>
          }
        />
      </List>
    </section>
  );
}

/**
 * The approval gate (a dry run): facts first (counts and space), then only
 * the questions that need the user, then the suggestions. Approve unlocks
 * once every question has an answer and the engine no longer blocks.
 */
export function RecvReviewScreen({ model, on }: RecvReviewScreenProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const left = unanswered(model.decisions);
  const blocked = left > 0 || model.blocked || model.busy;
  const note =
    left > 0
      ? t('receive.review.needs', { count: left })
      : model.blocked
        ? t('receive.review.blockedNote')
        : t('receive.review.ready', { size: format.bytes(model.rail.bytes) });
  const { counts } = model;
  const kept = retentionCopy(model.retention);
  return (
    <FlowLayout
      rail={<ReviewRail model={model} />}
      route={<RouteChip route={model.rail.route} here="to" />}
      actions={
        <ActionBar
          start={
            <Button kind="quiet" onClick={on.onCancel}>
              {t('receive.review.cancel')}
            </Button>
          }
          note={note}
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
        lede={t(`receive.review.lede.${kept.key}`, { count: kept.count })}
      />
      <section aria-label={t('receive.review.counts')}>
        <Stats>
          <Stat
            value={format.number(counts.added)}
            label={t('receive.review.added', { count: counts.added })}
          />
          <Stat value={format.number(counts.replaced)} label={t('receive.review.replaced')} />
          <Stat value={format.number(counts.identical)} label={t('receive.review.identical')} />
          <Stat value={format.number(counts.left)} label={t('receive.review.stayed')} />
        </Stats>
      </section>
      <SpaceRow space={model.space} onIgnore={on.onIgnoreSpace} />
      {model.decisions.length > 0 && <ReviewDecisions decisions={model.decisions} on={on} />}
      <Suggestions model={model} on={on} />
    </FlowLayout>
  );
}
