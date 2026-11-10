import { Trans, useTranslation } from 'react-i18next';
import {
  ActionBar,
  Banner,
  Button,
  CodeDisplay,
  CodeInput,
  Field,
  FlowCenter,
  FlowLayout,
  Handshake,
  Head,
  Input,
  Kbd,
  Note,
  Status,
} from '../../ui';
import { osName } from '../send/SendRail';
import { errorText } from '../../engine/errors';
import type { BridgeError } from '../../engine/types';
import { MoveRail } from '../shared/MoveRail';
import type { Computer } from '../shared/model';
import type { CodeEntryIntents, CodeEntryModel } from './model';

export interface RecvCodeScreenProps {
  readonly model: CodeEntryModel;
  /** This computer, the move's destination. */
  readonly here: Computer;
  readonly on: CodeEntryIntents;
}

/** Before pairing the rail knows one end: this computer. */
function CodeRail({ here, peer }: { readonly here: Computer; readonly peer: string | null }) {
  const { t } = useTranslation();
  return (
    <MoveRail
      route={{ from: { name: peer, os: null }, to: here }}
      here="to"
      notes={{ from: t('transfer.move.pairing'), to: t('transfer.move.here') }}
      facts={[]}
      promise={t('receive.code.promise')}
    />
  );
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
export function RecvCodeScreen({ model, here, on }: RecvCodeScreenProps) {
  const { t } = useTranslation();

  if (model.status === 'connecting') {
    return (
      <FlowLayout
        rail={<CodeRail here={here} peer={model.peer} />}
        actions={
          <ActionBar
            start={
              <Button kind="quiet" onClick={on.onCancel}>
                {t('action.cancel')}
              </Button>
            }
          />
        }
      >
        <Head
          center
          title={t('receive.code.connectingTitle')}
          lede={t('receive.code.connectingLede')}
        />
        <Handshake
          here={model.here}
          there={model.peer}
          linked={false}
          hereOs={here.os === null ? null : { kind: here.os, label: osName(here.os, t) }}
        />
        <FlowCenter>
          <CodeDisplay words={model.code} size="sm" />
          <Note center>{t('receive.code.typed')}</Note>
          <Status>{t('receive.code.checking', { peer: model.peer })}</Status>
        </FlowCenter>
      </FlowLayout>
    );
  }

  const notFound = model.status === 'not-found';
  const address = notFound ? model.address : '';
  return (
    <FlowLayout
      rail={<CodeRail here={here} peer={model.status === 'failed' ? null : model.peer} />}
      actions={
        <ActionBar
          start={
            <Button kind="quiet" icon="back" onClick={on.onBack}>
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
