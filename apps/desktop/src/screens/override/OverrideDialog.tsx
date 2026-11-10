import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Check, Dialog } from '../../ui';
import type { OverrideIntents, OverrideModel } from './model';
import './override.css';

interface OverrideText {
  readonly title: string;
  readonly body: readonly string[];
  /** The safe choice's label (default "Keep it safe"). */
  readonly keep?: string;
  readonly go: string;
  /** A statement the user ticks before the override unlocks. */
  readonly understand?: string;
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
    case 'cross_os': {
      const from = t(`settings.about.os.${model.from}`);
      const to = t(`settings.about.os.${model.to}`);
      return {
        title: t('override.cross_os.title', { item: model.item }),
        body: [t('override.cross_os.body', { from, to }), t('override.cross_os.safe')],
        keep: t('override.cross_os.keep'),
        go: t('override.cross_os.go'),
        understand: t('override.cross_os.understand', { to }),
      };
    }
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
 * a primary button that names the exact risk. The safe choice has focus; some
 * risks also need an explicit "I understand" before the button unlocks.
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
  const [understood, setUnderstood] = useState(false);
  return (
    <Dialog
      tone="warning"
      icon="warn"
      title={text.title}
      onDismiss={on.onKeepSafe}
      foot={
        <>
          <Button data-initial-focus onClick={on.onKeepSafe}>
            {text.keep ?? t('override.keepSafe')}
          </Button>
          <Button
            kind="primary"
            disabled={text.understand !== undefined && !understood}
            onClick={on.onOverride}
          >
            {text.go}
          </Button>
        </>
      }
    >
      {text.body.map((paragraph) => (
        <p key={paragraph}>{paragraph}</p>
      ))}
      {text.understand !== undefined && (
        <label className="s-understand">
          <Check value={understood} label={text.understand} onChange={setUnderstood} />
          <span aria-hidden="true">{text.understand}</span>
        </label>
      )}
    </Dialog>
  );
}
