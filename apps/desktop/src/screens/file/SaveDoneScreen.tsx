import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { ActionBar, Arrival, Button, CopyBox, FlowCenter, FlowLayout, Head } from '../../ui';
import type { SavedIntents, SavedModel } from './model';

/** Packed: the file's path to copy, and where to open it. */
export function SaveDoneScreen({
  model,
  on,
}: {
  readonly model: SavedModel;
  readonly on: SavedIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <FlowLayout
      actions={
        <ActionBar
          start={
            on.onShowInFolder && (
              <Button icon="folder" onClick={on.onShowInFolder}>
                {t('file.saved.showInFolder')}
              </Button>
            )
          }
          end={
            <Button kind="primary" onClick={on.onDone}>
              {t('action.done')}
            </Button>
          }
        />
      }
    >
      <FlowCenter>
        <Arrival
          word={t('file.saved.stamp')}
          meta={t('file.saved.meta', { size: format.bytes(model.bytes) })}
        />
        <Head center title={t('file.saved.title')} lede={t('file.saved.lede')} />
      </FlowCenter>
      <CopyBox text={model.path} />
    </FlowLayout>
  );
}
