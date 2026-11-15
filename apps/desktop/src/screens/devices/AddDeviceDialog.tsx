import { useTranslation } from 'react-i18next';
import { Button, CodeDisplay, Dialog, Note, Status } from '../../ui';
import { clock, useSecondsLeft } from '../send/countdown';
import { LinkedSummary } from './LinkedSummary';
import { deviceIcon, type AddDeviceIntents, type AddDeviceModel } from './model';
import './devices.css';

/** What to check when the device couldn't connect. */
const REASONS = ['wifi', 'guest', 'vpn'] as const;

/** "New code in 2:41": read by the eye; it is not a live region, so it never interrupts. */
function Expiry({ at }: { readonly at: number }) {
  const { t } = useTranslation();
  return <Note center>{t('ecosystem.pair.expiresIn', { time: clock(useSecondsLeft(at)) })}</Note>;
}

/**
 * Add a device: a QR code that carries the one-time code and this computer's
 * address, the three words as the fallback, then the device found (its key
 * to check: nothing links before the user confirms), then linked, expired
 * or failed.
 */
export function AddDeviceDialog({
  model,
  on,
}: {
  readonly model: AddDeviceModel;
  readonly on: AddDeviceIntents;
}) {
  const { t } = useTranslation();
  const cancel = (
    <Button data-initial-focus onClick={on.onCancel}>
      {t('ecosystem.pair.cancel')}
    </Button>
  );
  switch (model.state) {
    case 'starting':
    case 'waiting':
      return (
        <Dialog
          icon="phone"
          title={t('ecosystem.pair.title')}
          onDismiss={on.onCancel}
          foot={cancel}
        >
          {model.state === 'starting' ? (
            <Status>{t('ecosystem.pair.starting')}</Status>
          ) : (
            <>
              <p>{t('ecosystem.pair.lede')}</p>
              <img
                className="s-pair__qr"
                src={`data:image/svg+xml,${encodeURIComponent(model.invite.qr_svg)}`}
                alt={t('ecosystem.pair.qr')}
              />
              <Note center>{t('ecosystem.pair.orType')}</Note>
              <CodeDisplay words={model.invite.code.code.split('-')} size="sm" reveal />
              <Expiry at={model.expiresAt} />
            </>
          )}
        </Dialog>
      );
    case 'found': {
      const { peer } = model;
      const kind = peer.os === 'android' ? 'phone' : 'computer';
      return (
        <Dialog
          icon={deviceIcon(peer.os)}
          title={t(`ecosystem.pair.found.${kind}`)}
          onDismiss={on.onCancel}
          foot={
            <>
              <Button data-initial-focus onClick={on.onNotMine}>
                {t(`ecosystem.pair.notMine.${kind}`)}
              </Button>
              <Button kind="primary" onClick={on.onConfirm}>
                {t('ecosystem.pair.confirm')}
              </Button>
            </>
          }
        >
          <p>{t('ecosystem.pair.foundLede')}</p>
          <p>
            {t('ecosystem.pair.peer', {
              name: peer.name,
              os: t(`settings.about.os.${peer.os}`),
            })}
          </p>
          <p className="p-mono">{peer.fingerprint}</p>
        </Dialog>
      );
    }
    case 'expired':
      return (
        <Dialog
          icon="clock"
          title={t('ecosystem.pair.expiredTitle')}
          onDismiss={on.onCancel}
          foot={
            <>
              {cancel}
              <Button kind="primary" icon="refresh" onClick={on.onRetry}>
                {t('ecosystem.pair.newCode')}
              </Button>
            </>
          }
        >
          <p>{t('ecosystem.pair.expiredBody')}</p>
        </Dialog>
      );
    case 'failed':
      return (
        <Dialog
          tone="warning"
          icon="warn"
          title={t('ecosystem.pair.failedTitle')}
          onDismiss={on.onCancel}
          foot={
            <>
              {cancel}
              <Button kind="primary" onClick={on.onRetry}>
                {t('ecosystem.pair.retry')}
              </Button>
            </>
          }
        >
          <p>{t('ecosystem.pair.failedBody')}</p>
          <ul className="s-pair__reasons">
            {REASONS.map((reason) => (
              <li key={reason}>{t(`ecosystem.pair.reasons.${reason}`)}</li>
            ))}
          </ul>
        </Dialog>
      );
    case 'linked':
      return (
        <Dialog
          icon="check"
          title={
            model.device === null
              ? t('ecosystem.pair.linkedUnnamed')
              : t('ecosystem.pair.linkedTitle', { name: model.device.name })
          }
          onDismiss={on.onDone}
          foot={
            <>
              <Button onClick={on.onDone}>{t('ecosystem.pair.done')}</Button>
              <Button kind="primary" data-initial-focus onClick={on.onOpen}>
                {t('ecosystem.pair.open')}
              </Button>
            </>
          }
        >
          <LinkedSummary device={model.device} />
        </Dialog>
      );
  }
}
