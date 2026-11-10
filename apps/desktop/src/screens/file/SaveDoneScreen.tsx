import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { ActionBar, Button, CopyBox, FlowCenter, FlowLayout, Head, Seal } from '../../ui';
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
        <Seal ring={t('file.saved.ring')} label={t('file.saved.seal')} />
        <Head
          center
          title={t('file.saved.title')}
          lede={t('file.saved.lede', { size: format.bytes(model.bytes) })}
        />
      </FlowCenter>
      <CopyBox text={model.path} />
    </FlowLayout>
  );
}
