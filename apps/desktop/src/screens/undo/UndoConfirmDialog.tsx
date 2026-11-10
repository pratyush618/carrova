import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Button, Dialog, List, Note, Row } from '../../ui';
import type { UndoPreviewIntents, UndoPreviewModel } from './model';
import { UndoCalc } from './UndoCalc';

/** The undo plan, before anything happens. */
export function UndoConfirmDialog({
  model,
  on,
}: {
  readonly model: UndoPreviewModel;
  readonly on: UndoPreviewIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const { counts } = model;
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
      {counts === null ? (
        <p role="status">{t('undo.confirm.checking')}</p>
      ) : (
        <>
          <UndoCalc counts={counts} />
          <Note>{t('undo.confirm.keptAsideSub')}</Note>
          {counts.blockers.length > 0 && (
            <List kind="plain">
              {counts.blockers.map((b) => (
                <Row
                  key={b.what}
                  title={t('undo.confirm.blocked', { what: b.what })}
                  sub={b.reason}
                />
              ))}
            </List>
          )}
        </>
      )}
    </Dialog>
  );
}
