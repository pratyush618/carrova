import { useTranslation } from 'react-i18next';
import { useHost } from '../../../engine/host';
import { undoResultModel } from '../../../flows/history/historyViews';
import { UndoResultScreen } from '../../../screens/undo/UndoResultScreen';
import { BusyScreen, FailedScreen } from '../../../screens/shared/StatusScreens';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';

/** Undo's result: running, failed (with retry), or what happened to every file. */
export function UndoRoute() {
  const { t } = useTranslation();
  const { undo } = useControllers();
  const { revealPath } = useHost();
  const nav = useNav();
  const home = () => {
    nav.go({ type: 'closeFlow' });
  };
  const { rollback } = undo;
  switch (rollback.status) {
    case 'idle':
    case 'loading':
      return (
        <BusyScreen title={t('undo.result.working')} status={t('undo.result.workingStatus')} />
      );
    case 'failed':
      return <FailedScreen error={rollback.error} onRetry={undo.confirm} onHome={home} />;
    case 'ready': {
      const model = undoResultModel(rollback.value);
      const dir = model.movedAsideDir;
      return (
        <UndoResultScreen
          model={model}
          on={{
            onRetry: undo.confirm,
            ...(revealPath &&
              dir !== null && {
                onOpenMovedAside: () => {
                  void revealPath(dir);
                },
              }),
            onDone: home,
          }}
        />
      );
    }
  }
}
