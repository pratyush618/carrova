import { Trans, useTranslation } from 'react-i18next';
import {
  ActionBar,
  Banner,
  Button,
  CodeInput,
  Field,
  FlowLayout,
  Handshake,
  Head,
  Input,
  Kbd,
  Status,
} from '../../ui';
import { errorText } from '../../engine/errors';
import type { BridgeError } from '../../engine/types';
import type { CodeEntryIntents, CodeEntryModel } from './model';

export interface RecvCodeScreenProps {
  readonly model: CodeEntryModel;
  readonly on: CodeEntryIntents;
}

function FailedBanner({ error }: { readonly error: BridgeError }) {
  const { t } = useTranslation();
  const text = errorText(error, t);
  return <Banner tone="danger" title={text.title} body={text.body} />;
}

/** The code's leading number ("7" of "7-zebra-lemon"), for the not-found message. */
const codeNumber = (value: string): string => value.split('-')[0] ?? '';

/**
 * Words autocomplete after three letters; Tab accepts. Connecting is the
 * handshake moment. "Didn't match" tells the truth: the sender made a new code.
 */
export function RecvCodeScreen({ model, on }: RecvCodeScreenProps) {
  const { t } = useTranslation();

  if (model.status === 'connecting') {
    return (
      <FlowLayout
        actions={<ActionBar start={<Button onClick={on.onCancel}>{t('action.cancel')}</Button>} />}
      >
        <Head title={t('receive.code.connectingTitle')} lede={t('receive.code.connectingLede')} />
        <Handshake here={model.here} there={model.peer} linked={false} />
        <Status>{t('receive.code.checking', { peer: model.peer })}</Status>
      </FlowLayout>
    );
  }

  const notFound = model.status === 'not-found';
  const address = notFound ? model.address : '';
  return (
    <FlowLayout
      actions={
        <ActionBar
          start={
            <Button icon="back" onClick={on.onBack}>
              {t('action.back')}
            </Button>
          }
          end={
            <Button kind="primary" onClick={on.onConnect}>
              {notFound ? t('receive.code.connectAddress') : t('receive.code.connect')}
            </Button>
          }
        />
      }
    >
      <Head title={t('receive.code.title')} />
      <Field
        label={t('receive.code.label')}
        hint={<Trans i18nKey="receive.code.hint" components={{ kbd: <Kbd /> }} />}
      >
        {(control) => (
          <CodeInput
            control={control}
            value={model.value}
            words={model.words}
            onChange={on.onChange}
            onSubmit={on.onConnect}
          />
        )}
      </Field>
      {notFound && (
        <>
          <Banner
            tone="accent"
            title={t('receive.code.notFoundTitle', { number: codeNumber(model.value) })}
            body={t('receive.code.notFoundBody')}
          />
          <Field label={t('receive.code.address')}>
            {(control) => (
              <Input
                {...control}
                mono
                value={address}
                placeholder={t('receive.code.addressExample')}
                onChange={(event) => {
                  on.onAddress(event.target.value);
                }}
              />
            )}
          </Field>
        </>
      )}
      {model.status === 'failed' && <FailedBanner error={model.error} />}
      {model.status === 'mismatch' && (
        <Banner
          tone="danger"
          title={t('receive.code.mismatchTitle')}
          body={
            model.peer === null
              ? t('receive.code.mismatchBodyUnknown')
              : t('receive.code.mismatchBody', { peer: model.peer })
          }
        />
      )}
    </FlowLayout>
  );
}
