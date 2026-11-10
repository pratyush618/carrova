import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  Button,
  Disclose,
  Icon,
  List,
  Log,
  Row,
  Sheet,
  Timeline,
  type TimelineEntry,
} from '../../ui';
import type { SessionIntents, SessionModel, TimelineStep } from './model';
import { moveTitle } from './moveTitle';

function useTimeline(steps: readonly TimelineStep[]): TimelineEntry[] {
  const { t } = useTranslation();
  const format = useFormat();
  return steps.map((step) => ({
    id: step.kind,
    label:
      step.kind === 'started'
        ? t('history.timeline.started')
        : t(`history.timeline.state.${step.state}`),
    dateTime: step.at.toISOString(),
    time: format.time(step.at),
  }));
}

/**
 * Timeline, a redacted log, and what can still be done: resume an interrupted
 * move, or undo a finished one. Removing the undo point is quiet and confirmed.
 */
export function SessionSheet({
  model,
  on,
}: {
  readonly model: SessionModel;
  readonly on: SessionIntents;
}) {
  const { t } = useTranslation();
  const entries = useTimeline(model.timeline);
  const { undoable, resumable } = model.entry;
  return (
    <Sheet
      title={moveTitle(model.entry, t)}
      onClose={on.onClose}
      foot={
        <>
          {resumable && (
            <Button kind="primary" icon="swap" onClick={on.onResume}>
              {t('history.resume')}
            </Button>
          )}
          {undoable && (
            <Button kind="primary" icon="undo" onClick={on.onUndo}>
              {t('history.undo')}
            </Button>
          )}
          {!resumable && !undoable && <Button onClick={on.onClose}>{t('ui.close')}</Button>}
        </>
      }
    >
      <Timeline entries={entries} />
      {model.log.length > 0 && (
        <Disclose summary={t('history.log')}>
          <Log lines={model.log} />
        </Disclose>
      )}
      {(undoable || resumable) && (
        <List>
          <Row
            lead={<Icon name={undoable ? 'undo' : 'history'} />}
            title={undoable ? t('history.undoPoint') : t('history.staged')}
            sub={undoable ? t('history.undoPointSub') : t('history.stagedSub')}
            trail={
              <Button kind="quiet" size="sm" onClick={on.onRemoveUndoPoint}>
                {t('history.remove')}
              </Button>
            }
          />
        </List>
      )}
    </Sheet>
  );
}
