import { useTranslation } from 'react-i18next';
import { Decision, GroupHead, LinkButton, Note, type SegmentOption } from '../../ui';
import { askOptions } from '../shared/askOptions';
import type { DecisionAnswer, DecisionModel, ReviewIntents } from './model';

/** How many decisions still need an answer. Approve stays disabled until zero. */
export function unanswered(decisions: readonly DecisionModel[]): number {
  return decisions.filter((d) => d.answer === null).length;
}

type Intents = Pick<ReviewIntents, 'onAnswer' | 'onApplyWhileOpen'>;

function RunningDecision({
  decision,
  on,
}: {
  readonly decision: Extract<DecisionModel, { kind: 'app-running' }>;
  readonly on: Intents;
}) {
  const { t } = useTranslation();
  const options: readonly SegmentOption<DecisionAnswer>[] = [
    { value: 'skip', label: t('receive.review.skipApp', { app: decision.app }) },
    { value: 'closed', label: t('receive.review.closedIt') },
  ];
  return (
    <Decision
      icon="warn"
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

/** The short list of plain-words questions on the review screen. */
export function ReviewDecisions({
  decisions,
  on,
}: {
  readonly decisions: readonly DecisionModel[];
  readonly on: Intents;
}) {
  const { t } = useTranslation();
  const left = unanswered(decisions);
  const options: readonly SegmentOption<DecisionAnswer>[] = askOptions(t);
  return (
    <div>
      <GroupHead
        title={t('receive.review.decisions')}
        aside={
          <Note>
            {left > 0 ? t('receive.review.left', { count: left }) : t('receive.review.allAnswered')}
          </Note>
        }
      />
      {decisions.map((d) =>
        d.kind === 'secret' ? (
          <Decision
            key={d.key}
            icon="key"
            title={d.title}
            why={d.why}
            options={options}
            value={d.answer}
            onChange={(answer) => {
              on.onAnswer(d.key, answer);
            }}
          />
        ) : (
          <RunningDecision key={d.key} decision={d} on={on} />
        ),
      )}
    </div>
  );
}
