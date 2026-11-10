import { useTranslation } from 'react-i18next';
import { ActionBar, Banner, Button, FlowLayout, Head, Icon, List, Path, Row } from '../../ui';
import type { UndoResultIntents, UndoResultModel } from './model';

/** Same shape as every result: what happened, what's true now, one action. */
export function UndoResultScreen({
  model,
  on,
}: {
  readonly model: UndoResultModel;
  readonly on: UndoResultIntents;
}) {
  const { t } = useTranslation();
  const first = model.failures[0];
  return (
    <FlowLayout
      actions={
        <ActionBar
          end={
            <Button kind="primary" onClick={on.onDone}>
              {t('action.done')}
            </Button>
          }
        />
      }
    >
      {first === undefined ? (
        <Head
          title={t('undo.result.title')}
          lede={t('undo.result.lede', {
            restored: t('count.files', { count: model.restored }),
            removed: model.removed,
            movedAside: model.movedAside,
          })}
        />
      ) : (
        <>
          <Head
            title={t('undo.result.titlePartial')}
            lede={t('undo.result.ledePartial', { count: model.failures.length })}
          />
          <Banner
            tone="danger"
            title={t('undo.result.failed', { count: model.failures.length })}
            body={t('undo.result.failedBody', { what: first.what, reason: first.reason })}
            action={
              <Button size="sm" onClick={on.onRetry}>
                {t('action.tryAgain')}
              </Button>
            }
          />
        </>
      )}
      {model.movedAsideDir !== null && (
        <List>
          <Row
            lead={<Icon name="folder" />}
            title={t('undo.result.movedAside')}
            sub={<Path>{model.movedAsideDir}</Path>}
            trail={
              on.onOpenMovedAside && (
                <Button size="sm" onClick={on.onOpenMovedAside}>
                  {t('action.open')}
                </Button>
              )
            }
          />
        </List>
      )}
    </FlowLayout>
  );
}
