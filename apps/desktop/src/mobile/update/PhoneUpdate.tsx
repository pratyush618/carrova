import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import type { UpdateModel } from '../../screens/settings/model';
import { DownloadBar } from '../../screens/settings/UpdateDialog';
import { AppGlyph, Banner, Button, Card, LinkButton, Segmented, Status } from '../../ui';
import { PhonePage } from '../PhonePage';

/** What the user can do on the update page. */
export interface PhoneUpdateIntents {
  readonly onBack: () => void;
  /** Download (on mobile data only if Settings allow it). */
  readonly onDownload: () => void;
  /** Download on mobile data this once. */
  readonly onDownloadOnMobileData: () => void;
  readonly onCancel: () => void;
  readonly onPause: (paused: boolean) => void;
  /** Let the download waiting for Wi-Fi go on mobile data. */
  readonly onUseMobileData: () => void;
  readonly onOpenPermission: () => void;
  readonly onReleaseNotes: () => void;
}

/** Off Wi-Fi with mobile data off in Settings, the offer asks which way. */
type Network = 'wifi' | 'mobile';

/**
 * A phone's own update, one page per step: the offer and its notes, the
 * download (pause, resume, waiting for Wi-Fi), the signature check,
 * Android's install permission, then Android's installer, which asks.
 */
export function PhoneUpdate({
  model,
  current,
  askNetwork,
  on,
}: {
  readonly model: UpdateModel;
  /** The running version. */
  readonly current: string | null;
  /** Off Wi-Fi and mobile data is off in Settings: the offer asks. */
  readonly askNetwork: boolean;
  readonly on: PhoneUpdateIntents;
}) {
  const { t } = useTranslation();
  const [network, setNetwork] = useState<Network>('wifi');
  const version = 'version' in model ? model.version : null;
  return (
    <PhonePage title={t('mobile.update.title')} onBack={on.onBack}>
      {version !== null && (
        <div className="m-update-head">
          <AppGlyph glyph={{ icon: 'receive' }} size="sm" />
          <div>
            <b>{t('mobile.update.head', { version })}</b>
            {current !== null && (
              <p className="m-note">{t('mobile.update.youHave', { version: current })}</p>
            )}
          </div>
        </div>
      )}
      <Step
        model={model}
        current={current}
        askNetwork={askNetwork}
        network={network}
        onNetwork={setNetwork}
        on={on}
      />
    </PhonePage>
  );
}

function Step({
  model,
  current,
  askNetwork,
  network,
  onNetwork,
  on,
}: {
  readonly model: UpdateModel;
  readonly current: string | null;
  readonly askNetwork: boolean;
  readonly network: Network;
  readonly onNetwork: (network: Network) => void;
  readonly on: PhoneUpdateIntents;
}) {
  const { t } = useTranslation();
  const cancel = (
    <Button kind="quiet" onClick={on.onCancel}>
      {t('action.cancel')}
    </Button>
  );
  switch (model.status) {
    case 'checking':
      return <Status>{t('settings.update.checking')}</Status>;
    case 'up_to_date':
      return <Status>{t('settings.more.upToDate')}</Status>;
    case 'available':
      return (
        <>
          <Card title={t('mobile.update.whatsNew')}>
            {model.notes !== null && <p className="m-text">{model.notes}</p>}
            <LinkButton onClick={on.onReleaseNotes}>{t('mobile.update.fullNotes')}</LinkButton>
          </Card>
          {askNetwork && (
            <Segmented<Network>
              label={t('mobile.update.onMobile')}
              options={[
                { value: 'wifi', label: t('mobile.update.waitForWifi') },
                { value: 'mobile', label: t('mobile.update.useMobileData') },
              ]}
              value={network}
              onChange={onNetwork}
            />
          )}
          <Actions>
            <Button
              kind="primary"
              size="lg"
              onClick={network === 'mobile' ? on.onDownloadOnMobileData : on.onDownload}
            >
              {askNetwork && network === 'wifi'
                ? t('mobile.update.downloadOnWifi')
                : t('mobile.update.download')}
            </Button>
            <Button kind="quiet" onClick={on.onBack}>
              {t('mobile.update.later')}
            </Button>
          </Actions>
          <p className="m-note">{t('mobile.update.trust')}</p>
        </>
      );
    case 'downloading':
      return (
        <>
          <DownloadBar progress={model.progress} />
          <Actions>
            <Button
              size="lg"
              icon="pause"
              onClick={() => {
                on.onPause(true);
              }}
            >
              {t('settings.update.pause')}
            </Button>
            {cancel}
          </Actions>
        </>
      );
    case 'paused':
      return (
        <>
          <DownloadBar progress={model.progress} />
          <p className="m-note">{t('settings.update.pausedBody')}</p>
          <Actions>
            <Button
              kind="primary"
              size="lg"
              icon="play"
              onClick={() => {
                on.onPause(false);
              }}
            >
              {t('settings.update.resume')}
            </Button>
            {cancel}
          </Actions>
        </>
      );
    case 'waiting':
      return (
        <>
          <Banner
            tone="warning"
            icon="wifi"
            title={t('mobile.update.wifiDropped')}
            body={t('mobile.update.wifiDroppedBody')}
          />
          <Actions>
            <Button kind="primary" size="lg" onClick={on.onUseMobileData}>
              {t('mobile.update.useMobileData')}
            </Button>
            {cancel}
          </Actions>
        </>
      );
    case 'verifying':
      return <Status>{t('settings.update.verifying')}</Status>;
    case 'permission':
      return (
        <>
          <Banner
            tone="warning"
            icon="lock"
            title={t('settings.update.permission.title')}
            body={t('settings.update.permission.body')}
          />
          <ol className="m-steps">
            <li>
              <b>{t('mobile.update.steps.one')}</b>
              <span>{t('mobile.update.steps.oneSub')}</span>
            </li>
            <li>
              <b>{t('mobile.update.steps.two')}</b>
            </li>
            <li>
              <b>{t('mobile.update.steps.three')}</b>
              <span>{t('mobile.update.steps.threeSub')}</span>
            </li>
          </ol>
          <Actions>
            <Button kind="primary" size="lg" onClick={on.onOpenPermission}>
              {t('mobile.update.openSettings')}
            </Button>
            {cancel}
          </Actions>
          <p className="m-note">{t('mobile.update.permissionFoot')}</p>
        </>
      );
    case 'installed':
      return <Status>{t('settings.update.installed')}</Status>;
    case 'installing':
    case 'handed':
      return (
        <>
          <Status>{t('settings.update.handed')}</Status>
          <Actions>
            <Button kind="primary" size="lg" onClick={on.onDownload}>
              {t('mobile.update.openInstaller')}
            </Button>
          </Actions>
        </>
      );
    case 'failed': {
      const text = errorText(model.error, t);
      return (
        <>
          <Banner
            tone="danger"
            title={text.title}
            body={
              current === null
                ? text.body
                : `${text.body} ${t('mobile.update.unchanged', { version: current })}`
            }
          />
          {model.version !== null && (
            <Actions>
              <Button kind="primary" size="lg" onClick={on.onDownload}>
                {t('settings.update.again')}
              </Button>
            </Actions>
          )}
        </>
      );
    }
  }
}

function Actions({ children }: { readonly children: ReactNode }) {
  return <div className="m-wide">{children}</div>;
}
