import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Banner, Button, Disclose, Log, Muted, Timeline, type TimelineEntry } from '../../ui';
import { UndoCalc } from '../undo/UndoCalc';
import type { HistoryIntents, SessionModel } from './model';
import { moveTitle } from './moveWords';

type DetailIntents = Pick<HistoryIntents, 'onUndo' | 'onResume' | 'onForget'>;

function useSteps(model: SessionModel, now: Date): TimelineEntry[] {
  const { t } = useTranslation();
  const format = useFormat();
  const until = model.undoUntil;
  const undone = model.entry.kind === 'undone';
  const steps: TimelineEntry[] = model.timeline.map((step) => ({
    id: step.kind,
    label:
      step.kind === 'started'
        ? t('history.timeline.started')
        : t(`history.timeline.state.${step.state}`),
    dateTime: step.at.toISOString(),
    time: t('summary.dayTime', { day: format.day(step.at, now, true), time: format.time(step.at) }),
    ...(undone && { tone: 'muted' as const }),
  }));
  if (model.entry.undoable)
    steps.push({
      id: 'undo-point',
      label: t('history.undoPoint'),
      detail:
        until === null
          ? t('history.undoPointSub')
          : t('history.undoUntil', { date: format.date(until) }),
      tone: 'accent',
    });
  // A rewind reads newest first.
  return undone ? steps.reverse() : steps;
}

/** The selected move, step by step, with what can still be done about it. */
export function MoveDetail({
  model,
  now,
  on,
}: {
  readonly model: SessionModel;
  readonly now: Date;
  readonly on: DetailIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const steps = useSteps(model, now);
  const { entry } = model;
  const undone = entry.kind === 'undone';
  return (
    <>
      {undone ? (
        <Banner tone="success" title={t('history.undone.title')} body={t('history.undone.body')} />
      ) : (
        <div>
          <h2 className="s-hist__h">{t('history.steps')}</h2>
          <Muted>
            {t('history.detailSub', {
              title: moveTitle(entry, t),
              day: format.day(entry.when, now),
            })}
          </Muted>
        </div>
      )}
      {undone && <h2 className="s-hist__h">{t('history.rewound')}</h2>}
      <Timeline entries={steps} rewind={undone} />
      {entry.resumable && (
        <Button kind="primary" icon="swap" onClick={on.onResume}>
          {t('history.resume')}
        </Button>
      )}
      {entry.undoable && model.undoCounts !== null && <UndoCalc counts={model.undoCounts} brief />}
      {entry.undoable && (
        <Button icon="undo" onClick={on.onUndo}>
          {t('history.undo')}
        </Button>
      )}
      {model.log.length > 0 && (
        <Disclose summary={t('history.log')}>
          <Log lines={model.log} />
        </Disclose>
      )}
      {(entry.undoable || entry.resumable) && (
        <Button kind="quiet" size="sm" onClick={on.onForget}>
          {t('history.forgetAction')}
        </Button>
      )}
    </>
  );
}
