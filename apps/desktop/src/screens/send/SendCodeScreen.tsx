import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import {
  ActionBar,
  Banner,
  Button,
  CodeDisplay,
  CopyBox,
  Disclose,
  Empty,
  FlowCenter,
  FlowLayout,
  Head,
  LockLine,
  Note,
  Stack,
  Status,
} from '../../ui';
import type { PairingIntents, PairingModel } from './model';

export interface SendCodeScreenProps {
  readonly model: PairingModel;
  readonly on: PairingIntents;
}

/**
 * The signature moment: the code flips in like a departure board. Every error
 * state has one action; the fallback address sits behind "Can't connect?".
 */
export function SendCodeScreen({ model, on }: SendCodeScreenProps) {
  const { t } = useTranslation();

  if (model.status === 'failed') {
    const text = errorText(model.error, t);
    return (
      <FlowLayout
        actions={<ActionBar start={<Button onClick={on.onHome}>{t('action.backHome')}</Button>} />}
      >
        <Empty
          art="lock"
          tone="danger"
          title={text.title}
          body={text.body}
          action={
            <Button kind="primary" onClick={on.onNewCode}>
              {t('action.tryAgain')}
            </Button>
          }
        />
      </FlowLayout>
    );
  }

  if (model.status === 'locked') {
    return (
      <FlowLayout
        actions={<ActionBar start={<Button onClick={on.onHome}>{t('action.backHome')}</Button>} />}
      >
        <Empty
          art="lock"
          tone="danger"
          title={t('send.code.lockedTitle')}
          body={t('send.code.lockedBody')}
          action={
            <Button kind="primary" onClick={on.onNewCode}>
              {t('send.code.startAgain')}
            </Button>
          }
        />
      </FlowLayout>
    );
  }

  if (model.status === 'preparing') {
    return (
      <FlowLayout
        actions={
          <ActionBar
            start={
              <Button icon="back" onClick={on.onBack}>
                {t('action.back')}
              </Button>
            }
          />
        }
      >
        <FlowCenter>
          <Head center title={t('send.code.preparingTitle')} />
          <Status>{t('send.code.preparing')}</Status>
        </FlowCenter>
      </FlowLayout>
    );
  }

  const cantConnect = model.status === 'cant-connect';
  const address = model.address;
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
            <Button kind="quiet" onClick={on.onNewCode}>
              {t('send.code.newCode')}
            </Button>
          }
        />
      }
    >
      <FlowCenter>
        <Head center title={t('send.code.title')} />
        {model.status === 'renewed' && (
          <Banner
            tone="warning"
            title={t('send.code.renewedTitle')}
            body={t('send.code.renewedBody')}
          />
        )}
        <CodeDisplay words={model.code} reveal />
        {!cantConnect && <Status>{t('send.code.waiting')}</Status>}
        <LockLine>{t('send.code.lock')}</LockLine>
        {address !== null &&
          (cantConnect ? (
            <Stack>
              <Banner
                tone="accent"
                title={t('send.code.cantFindTitle')}
                body={t('send.code.cantFindBody')}
              />
              <CopyBox text={address} />
            </Stack>
          ) : (
            <Disclose summary={t('send.code.cantConnect')}>
              <Stack>
                <Note>{t('send.code.addressHint')}</Note>
                <CopyBox text={address} />
              </Stack>
            </Disclose>
          ))}
      </FlowCenter>
    </FlowLayout>
  );
}
