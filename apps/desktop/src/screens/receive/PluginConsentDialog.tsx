import { useTranslation } from 'react-i18next';
import { Button, Dialog, GroupHead, PermRow, Stack } from '../../ui';
import type { PluginConsentIntents, PluginConsentModel } from './model';

/**
 * Asked on this computer, where the plugin runs: what it is, what it changes
 * and, one line each, what it may do. Allowed for this move only.
 */
export function PluginConsentDialog({
  model,
  on,
}: {
  readonly model: PluginConsentModel;
  readonly on: PluginConsentIntents;
}) {
  const { t } = useTranslation();
  const what = model.items.join(', ');
  return (
    <Dialog
      icon="plug"
      title={t('receive.plugin.title', { plugin: model.plugin })}
      onDismiss={on.onClose}
      foot={
        <>
          <Button data-initial-focus onClick={on.onDecline}>
            {t('receive.plugin.decline')}
          </Button>
          <Button kind="primary" onClick={on.onAllow}>
            {t('receive.plugin.allow')}
          </Button>
        </>
      }
    >
      <Stack>
        <p>{t('receive.plugin.sub', { version: model.version })}</p>
        <p>{model.description}</p>
        <div>
          <GroupHead title={t('receive.plugin.changes')} />
          <p>{what}</p>
        </div>
        <div>
          <GroupHead title={t('receive.plugin.may')} />
          <ul>
            {model.permissions.map((sentence) => (
              <PermRow key={sentence} icon="shield" title={sentence} detail={null} />
            ))}
          </ul>
        </div>
        <p>{t('receive.plugin.declineNote', { what })}</p>
      </Stack>
    </Dialog>
  );
}
