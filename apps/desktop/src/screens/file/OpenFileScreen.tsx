import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import { ActionBar, Banner, Button, Field, FlowLayout, Head, Input } from '../../ui';
import type { OpenFileIntents, OpenFileModel } from './model';

/** File and passphrase on one screen; then the receive review, progress and done. */
export function OpenFileScreen({
  model,
  on,
}: {
  readonly model: OpenFileModel;
  readonly on: OpenFileIntents;
}) {
  const { t } = useTranslation();
  const ready = model.path.trim() !== '' && model.passphrase !== '' && !model.opening;
  const failure = model.error === null ? null : errorText(model.error, t);
  return (
    <FlowLayout
      actions={
        <ActionBar
          start={
            <Button kind="quiet" icon="back" onClick={on.onBack}>
              {t('action.back')}
            </Button>
          }
          end={
            <Button kind="primary" disabled={!ready} onClick={on.onOpen}>
              {model.opening ? t('file.open.opening') : t('file.open.open')}
            </Button>
          }
        />
      }
    >
      <Head title={t('file.open.title')} />
      <Field label={t('file.open.file')} hint={t('file.open.fileHint')}>
        {(control) => (
          <Input
            {...control}
            mono
            value={model.path}
            placeholder={t('file.open.filePlaceholder')}
            onChange={(event) => {
              on.onPath(event.target.value);
            }}
          />
        )}
      </Field>
      {model.canChoose && (
        <div>
          <Button size="sm" icon="file" onClick={on.onChooseAnother}>
            {t('file.open.another')}
          </Button>
        </div>
      )}
      <Field
        label={t('file.open.passphrase')}
        {...(model.wrong && { error: t('file.open.wrong') })}
      >
        {(control) => (
          <Input
            {...control}
            type="password"
            autoComplete="current-password"
            value={model.passphrase}
            onChange={(event) => {
              on.onPassphrase(event.target.value);
            }}
          />
        )}
      </Field>
      {failure && <Banner tone="danger" title={failure.title} body={failure.body} />}
    </FlowLayout>
  );
}
