import { useTranslation } from 'react-i18next';
import { Button, Dialog, Icon, List, Note, Path, Row } from '../../ui';
import type { KeyConfirmIntents, KeyFile, SecretItem } from './model';

/** One key file: its path, then fingerprint · type · comment to tell it apart. */
function KeyRow({ file }: { readonly file: KeyFile }) {
  const { t } = useTranslation();
  return (
    <Row
      lead={<Icon name="key" size="sm" />}
      title={<Path>{file.path}</Path>}
      sub={
        <span className="s-send__fp">
          <Path>{file.fingerprint}</Path>
          {` · ${t(`send.keys.kind.${file.kind}`)}`}
          {file.comment !== null && ` · ${file.comment}`}
        </span>
      }
    />
  );
}

/**
 * "Bring your SSH keys?": one secret, its key files (with fingerprints when
 * recognised) and what a copy lets someone do. Leaving it behind is the safe
 * choice and has focus.
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
      <p>{t(`sensitive.${secret.why}`)}</p>
      <p>{t('send.keys.travel')}</p>
      <List label={t('send.keys.files')}>
        {secret.keys.map((file) => (
          <KeyRow key={file.path} file={file} />
        ))}
        {secret.paths
          .filter((path) => !secret.keys.some((file) => file.path === path))
          .map((path) => (
            <Row key={path} lead={<Icon name="key" size="sm" />} title={<Path>{path}</Path>} />
          ))}
      </List>
      <Note>{t('send.keys.asksAgain')}</Note>
    </Dialog>
  );
}
