import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import type { KdfStrength } from '../../engine/types';
import {
  ActionBar,
  Banner,
  Button,
  Field,
  FlowLayout,
  Head,
  Input,
  Meter,
  Note,
  Select,
} from '../../ui';
import type { SaveLockIntents, SaveLockModel } from './model';

/** The engine's levels, weakest (its default) first: none is below the default. */
const KDF_LEVELS: readonly KdfStrength[] = ['standard', 'strong', 'maximum'];

/** Strength is shown, not lectured. Where the file goes is one field; the lock is always strong. */
export function SaveLockScreen({
  model,
  on,
}: {
  readonly model: SaveLockModel;
  readonly on: SaveLockIntents;
}) {
  const { t } = useTranslation();
  const mismatch = model.confirm !== model.passphrase;
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
            <Button
              kind="primary"
              busy={model.saving}
              disabled={!model.canSave}
              onClick={on.onSave}
            >
              {model.saving ? t('file.lock.saving') : t('file.lock.save')}
            </Button>
          }
        />
      }
    >
      <Head title={t('file.lock.title')} lede={t('file.lock.lede')} />
      <Field
        label={t('file.lock.passphrase')}
        hint={t(`file.lock.strength.${model.strength}`)}
        below={<Meter strength={model.strength} />}
      >
        {(control) => (
          <Input
            {...control}
            type="password"
            autoComplete="new-password"
            value={model.passphrase}
            onChange={(event) => {
              on.onPassphrase(event.target.value);
            }}
          />
        )}
      </Field>
      <Field
        label={t('file.lock.again')}
        {...(model.confirm !== '' && mismatch && { error: t('file.lock.mismatch') })}
      >
        {(control) => (
          <Input
            {...control}
            type="password"
            autoComplete="new-password"
            value={model.confirm}
            onChange={(event) => {
              on.onConfirm(event.target.value);
            }}
          />
        )}
      </Field>
      <Field label={t('file.lock.where')}>
        {(control) => (
          <Input
            {...control}
            mono
            value={model.path}
            onChange={(event) => {
              on.onPath(event.target.value);
            }}
          />
        )}
      </Field>
      {model.canChoose && (
        <div>
          <Button size="sm" icon="folder" onClick={on.onChoosePath}>
            {t('file.lock.choose')}
          </Button>
        </div>
      )}
      <Field label={t('file.lock.kdf')} hint={t(`file.lock.kdfLevels.${model.kdf}.consequence`)}>
        {() => (
          <Select
            label={t('file.lock.kdf')}
            options={KDF_LEVELS.map((value) => ({
              value,
              label: t(`file.lock.kdfLevels.${value}.label`),
            }))}
            value={model.kdf}
            onChange={on.onKdf}
          />
        )}
      </Field>
      <Note>{t('file.lock.kdfNote')}</Note>
      {failure && <Banner tone="danger" title={failure.title} body={failure.body} />}
    </FlowLayout>
  );
}
