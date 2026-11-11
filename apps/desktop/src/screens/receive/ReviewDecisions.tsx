import { useTranslation } from 'react-i18next';
import {
  Badge,
  Button,
  Decision,
  GroupHead,
  LinkButton,
  List,
  Note,
  type SegmentOption,
} from '../../ui';
import { askOptions } from '../shared/askOptions';
import type { DecisionAnswer, DecisionModel, ReviewIntents } from './model';

/**
 * How many decisions still need an answer. Approve stays disabled until zero.
 * Accept all doesn't count: the picks it covers ask on their own rows.
 */
export function unanswered(decisions: readonly DecisionModel[]): number {
  return decisions.filter((d) => d.kind !== 'safe-picks' && d.answer === null).length;
}

type Intents = Pick<
  ReviewIntents,
  'onAnswer' | 'onAcceptSafePicks' | 'onApplyWhileOpen' | 'onBringAnyway' | 'onReviewPlugin'
>;

type Of<K extends DecisionModel['kind']> = Extract<DecisionModel, { kind: K }>;

function RunningDecision({
  decision,
  on,
}: {
  readonly decision: Of<'app-running'>;
  readonly on: Intents;
}) {
  const { t } = useTranslation();
  const options: readonly SegmentOption<DecisionAnswer>[] = [
    { value: 'skip', label: t('receive.review.skipApp', { app: decision.app }) },
    { value: 'closed', label: t('receive.review.closedIt') },
  ];
  return (
    <Decision
      icon="running"
      title={t('receive.review.running', { app: decision.app })}
      why={
        <>
          {t('receive.review.runningWhy', { app: decision.app })}{' '}
          <LinkButton
            onClick={() => {
              on.onApplyWhileOpen(decision.key);
            }}
          >
            {t('receive.review.applyWhileOpen')}
          </LinkButton>
        </>
      }
      options={options}
      value={decision.answer}
      onChange={(answer) => {
        on.onAnswer(decision.key, answer);
      }}
    />
  );
}

/** An item made for the other system: leave it, or override (behind its warning). */
function CrossOsDecision({
  decision,
  on,
}: {
  readonly decision: Of<'cross-os'>;
  readonly on: Intents;
}) {
  const { t } = useTranslation();
  const os = (kind: typeof decision.from) => t(`settings.about.os.${kind}`);
  const what = decision.title;
  return (
    <Decision
      icon="swap"
      title={t('receive.review.crossOs', { what, from: os(decision.from) })}
      why={t('receive.review.crossOsWhy', { from: os(decision.from), to: os(decision.to) })}
      answered={decision.answer !== null}
      action={
        <>
          {decision.answer === 'bring' && (
            <Badge tone="warning">{t('receive.review.broughtAnyway')}</Badge>
          )}
          {decision.answer !== 'bring' && (
            <LinkButton
              aria-label={t('receive.review.overrideFor', { what })}
              onClick={() => {
                on.onBringAnyway(decision.key);
              }}
            >
              {t('receive.review.override')}
            </LinkButton>
          )}
          {decision.answer === 'leave' ? (
            <Badge>{t('receive.review.leftBehind')}</Badge>
          ) : (
            <Button
              size="sm"
              aria-label={t('receive.review.leaveFor', { what })}
              onClick={() => {
                on.onAnswer(decision.key, 'leave');
              }}
            >
              {t('receive.review.leaveBehind')}
            </Button>
          )}
        </>
      }
    />
  );
}

/** A plugin that would change items' files: its permissions open in the consent dialog. */
function PluginDecision({
  decision,
  on,
}: {
  readonly decision: Of<'plugin'>;
  readonly on: Intents;
}) {
  const { t } = useTranslation();
  return (
    <Decision
      icon="plug"
      title={t('receive.review.plugin', {
        plugin: decision.key,
        what: decision.items.join(', '),
      })}
      why={decision.description}
      answered={decision.answer !== null}
      action={
        <>
          {decision.answer === 'allow' && (
            <Badge tone="success">{t('receive.review.pluginAllowed')}</Badge>
          )}
          {decision.answer === 'leave' && <Badge>{t('receive.review.leftBehind')}</Badge>}
          <Button
            size="sm"
            aria-label={t('receive.review.pluginReviewFor', { plugin: decision.key })}
            onClick={() => {
              on.onReviewPlugin(decision.key);
            }}
          >
            {t('receive.review.pluginReview')}
          </Button>
        </>
      }
    />
  );
}

/** The sender's own picks with nothing risky about them: tick them all at once. */
function SafePicksDecision({
  decision,
  on,
}: {
  readonly decision: Of<'safe-picks'>;
  readonly on: Intents;
}) {
  const { t } = useTranslation();
  return (
    <Decision
      icon="folder"
      title={t('receive.review.safePicks')}
      why={t('receive.review.safePicksWhy')}
      answered={decision.answer !== null}
      action={
        decision.answer === null ? (
          <Button size="sm" onClick={on.onAcceptSafePicks}>
            {t('receive.review.acceptAll', { count: decision.count })}
          </Button>
        ) : (
          <Badge tone="success">{t('receive.review.accepted')}</Badge>
        )
      }
    />
  );
}

function DecisionRow({ decision, on }: { readonly decision: DecisionModel; readonly on: Intents }) {
  const { t } = useTranslation();
  switch (decision.kind) {
    case 'safe-picks':
      return <SafePicksDecision decision={decision} on={on} />;
    case 'secret':
      return (
        <Decision
          icon="key"
          sensitive
          title={decision.title}
          why={decision.why}
          options={askOptions(t)}
          value={decision.answer}
          warn={decision.answer === 'bring'}
          onChange={(answer) => {
            on.onAnswer(decision.key, answer);
          }}
        />
      );
    case 'app-running':
      return <RunningDecision decision={decision} on={on} />;
    case 'cross-os':
      return <CrossOsDecision decision={decision} on={on} />;
    case 'plugin':
      return <PluginDecision decision={decision} on={on} />;
  }
}

/** The questions only the user can answer, each with its consequence in plain words. */
export function ReviewDecisions({
  decisions,
  on,
}: {
  readonly decisions: readonly DecisionModel[];
  readonly on: Intents;
}) {
  const { t } = useTranslation();
  const left = unanswered(decisions);
  const heading = left > 0 ? t('receive.review.needsAnswer') : t('receive.review.answered');
  return (
    <section aria-label={heading}>
      <GroupHead
        title={heading}
        aside={
          <Note>
            {left > 0 ? t('receive.review.left', { count: left }) : t('receive.review.allAnswered')}
          </Note>
        }
      />
      <List>
        {decisions.map((d) => (
          <li key={`${d.kind}:${d.key}`}>
            <DecisionRow decision={d} on={on} />
          </li>
        ))}
      </List>
    </section>
  );
}
