import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Button, Dialog, List, Row } from '../../ui';
import type { UndoPreviewIntents, UndoPreviewModel } from './model';

/** Restored, removed, moved aside: the undo plan, before anything happens. */
export function UndoConfirmDialog({
  model,
  on,
}: {
  readonly model: UndoPreviewModel;
  readonly on: UndoPreviewIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <Dialog
      icon="undo"
      title={t('undo.confirm.title')}
      onDismiss={on.onKeep}
      foot={
        <>
          <Button onClick={on.onKeep}>{t('undo.confirm.keep')}</Button>
          <Button kind="primary" onClick={on.onUndo}>
            {t('undo.confirm.go')}
          </Button>
        </>
      }
    >
      <p>
        {t('undo.confirm.body', {
          day: format.day(model.pointAt, model.now),
          time: format.time(model.pointAt),
        })}
      </p>
      <List kind="plain">
        {model.counts && (
          <>
            <Row
              title={t('undo.confirm.restored', { count: model.counts.restored })}
              sub={t('undo.confirm.restoredSub')}
            />
            <Row
              title={t('undo.confirm.removed', { count: model.counts.removed })}
              sub={t('undo.confirm.removedSub')}
            />
            {model.counts.movedAside > 0 && (
              <Row
                title={t('undo.confirm.movedAside', { count: model.counts.movedAside })}
                sub={t('undo.confirm.movedAsideSub')}
              />
            )}
            {model.counts.blockers.map((b) => (
              <Row
                key={b.what}
                title={t('undo.confirm.blocked', { what: b.what })}
                sub={b.reason}
              />
            ))}
          </>
        )}
        {model.counts === null && <Row title={t('undo.confirm.checking')} />}
        <Row title={t('undo.confirm.changedSince')} sub={t('undo.confirm.movedAsideSub')} />
      </List>
    </Dialog>
  );
}
