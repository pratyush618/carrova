import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import {
  ActionBar,
  Banner,
  Button,
  Card,
  CodeDisplay,
  CopyBox,
  Disclose,
  Empty,
  FlowCenter,
  FlowLayout,
  Head,
  LinkButton,
  Note,
  Stack,
  Status,
} from '../../ui';
import { clock, minutesLeft, useSecondsLeft } from './countdown';
import type { PairingIntents, PairingModel, SendMove } from './model';
import { RouteChip, SendRail } from './SendRail';

/** "New code in 4:52" for the eye; whole minutes, once a minute, for screen readers. */
function Countdown({ expiresAt }: { readonly expiresAt: number }) {
  const { t } = useTranslation();
  const seconds = useSecondsLeft(expiresAt);
  const minutes = minutesLeft(seconds);
  return (
    <>
      <span aria-hidden="true">{t('send.code.newCodeIn', { time: clock(seconds) })}</span>
      <span className="p-vh" role="status">
        {minutes > 0 ? t('send.code.changesIn', { count: minutes }) : t('send.code.changesSoon')}
      </span>
    </>
  );
}

export interface SendCodeScreenProps {
  readonly model: PairingModel;
  readonly move: SendMove;
  readonly on: PairingIntents;
}

/**
 * The signature moment: the code flips in like a departure board. Every error
 * state has one action; the fallback address sits behind "Can't connect?".
 */
export function SendCodeScreen({ model, move, on }: SendCodeScreenProps) {
  const { t } = useTranslation();

  if (model.status === 'failed') {
    const text = errorText(model.error, t);
    return (
      <FlowLayout
        actions={
          <ActionBar
            start={
              <Button kind="quiet" onClick={on.onHome}>
                {t('action.backHome')}
              </Button>
            }
          />
        }
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
        actions={
          <ActionBar
            start={
              <Button kind="quiet" onClick={on.onHome}>
                {t('action.backHome')}
              </Button>
            }
          />
        }
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

  if (model.status === 'preparing' || model.status === 'checking') {
    const checking = model.status === 'checking';
    return (
      <FlowLayout
        actions={
          <ActionBar
            start={
              <Button kind="quiet" icon="back" onClick={on.onBack}>
                {t('action.back')}
              </Button>
            }
          />
        }
      >
        <FlowCenter>
          <Head
            center
            title={checking ? t('send.code.checkingTitle') : t('send.code.preparingTitle')}
          />
          <Status>{checking ? t('send.code.checking') : t('send.code.preparing')}</Status>
        </FlowCenter>
      </FlowLayout>
    );
  }

  const cantConnect = model.status === 'cant-connect';
  const address = model.address;
  return (
    <FlowLayout
      route={<RouteChip here={move.here} />}
      {...(move.facts !== null && { rail: <SendRail move={move} purpose="send" /> })}
      actions={
        <ActionBar
          start={
            <Button kind="quiet" icon="back" onClick={on.onBack}>
              {t('action.back')}
            </Button>
          }
        />
      }
    >
      <Head title={t('send.code.title')} lede={t('send.code.lede')} />
      {model.status === 'renewed' && (
        <Banner
          tone="warning"
          title={t('send.code.renewedTitle')}
          body={t('send.code.renewedBody')}
        />
      )}
      <Card>
        <FlowCenter>
          {model.status === 'expired' && (
            <Note center>
              <span role="status">{t('send.code.expired')}</span>
            </Note>
          )}
          <CodeDisplay words={model.code} reveal />
          {/* No Copy link on purpose: the code stays off the clipboard (and its history). */}
          <Note center>
            <Countdown expiresAt={model.expiresAt} />{' '}
            <LinkButton onClick={on.onNewCode}>{t('send.code.newCode')}</LinkButton>
          </Note>
          {!cantConnect && <Status>{t('send.code.waiting')}</Status>}
        </FlowCenter>
      </Card>
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
    </FlowLayout>
  );
}
