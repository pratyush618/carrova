import { useTranslation } from 'react-i18next';
import { Button, Dialog, Icon, List, Note, Path, Row } from '../../ui';
import type { KeyConfirmIntents, SecretItem } from './model';

/**
 * "Bring your SSH keys?": one secret, its key files and what a copy lets
 * someone do. Leaving it behind is the safe choice and has focus.
 * Fingerprints are left out until the engine supplies them.
 */
export function KeyConfirmDialog({
  secret,
  on,
}: {
  readonly secret: SecretItem;
  readonly on: KeyConfirmIntents;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      tone="warning"
      icon="key"
      title={t('send.keys.title', { item: secret.label })}
      onDismiss={on.onLeave}
      foot={
        <>
          <Button data-initial-focus onClick={on.onLeave}>
            {t('send.ask.leave')}
          </Button>
          <Button kind="primary" onClick={on.onBring}>
            {t('send.ask.bring')}
          </Button>
        </>
      }
    >
      <p>{secret.why ?? t('send.found.secretWhy')}</p>
      <p>{t('send.keys.travel')}</p>
      <List label={t('send.keys.files')}>
        {secret.paths.map((path) => (
          <Row key={path} lead={<Icon name="key" size="sm" />} title={<Path>{path}</Path>} />
        ))}
      </List>
      <Note>{t('send.keys.asksAgain')}</Note>
    </Dialog>
  );
}
