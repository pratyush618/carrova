import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '../../ui';
import type { OverrideIntents, OverrideModel } from './model';

interface OverrideText {
  readonly title: string;
  readonly body: readonly string[];
  readonly go: string;
}

function useOverrideText(model: OverrideModel): OverrideText {
  const { t } = useTranslation();
  switch (model.kind) {
    case 'unmovable':
      return {
        title: t('override.unmovable.title', { item: model.item }),
        body: [model.reason, t('override.unmovable.suggestion', { count: model.count })],
        go: t('override.unmovable.go'),
      };
    case 'running':
      return {
        title: t('override.running.title', { app: model.app }),
        body: [t('override.running.body', { app: model.app }), t('override.running.safe')],
        go: t('override.running.go'),
      };
    case 'space':
      return {
        title: t('override.space.title'),
        body: [t('override.space.body'), t('override.space.safe')],
        go: t('override.space.go'),
      };
    case 'elsewhere':
      return {
        title: t('override.elsewhere.title'),
        body: [t('override.elsewhere.body'), t('override.elsewhere.safe')],
        go: t('override.elsewhere.go'),
      };
  }
}

/**
 * Every override uses this one dialog: what happens, what's still safe, and
 * a button that names the exact risk. The safe choice has focus.
 */
export function OverrideDialog({
  model,
  on,
}: {
  readonly model: OverrideModel;
  readonly on: OverrideIntents;
}) {
  const { t } = useTranslation();
  const text = useOverrideText(model);
  return (
    <Dialog
      tone="warning"
      icon="warn"
      title={text.title}
      onDismiss={on.onKeepSafe}
      foot={
        <>
          <Button kind="primary" data-initial-focus onClick={on.onKeepSafe}>
            {t('override.keepSafe')}
          </Button>
          <Button kind="danger" onClick={on.onOverride}>
            {text.go}
          </Button>
        </>
      }
    >
      {text.body.map((paragraph) => (
        <p key={paragraph}>{paragraph}</p>
      ))}
    </Dialog>
  );
}
