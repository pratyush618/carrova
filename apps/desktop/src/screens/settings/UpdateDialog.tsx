import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import type { DownloadView, UpdateBlocker } from '../../engine/types';
import { useFormat } from '../../i18n/useFormat';
import { Button, Dialog, LockLine, Progress, Status } from '../../ui';
import { useTimeLeft } from '../shared/TransferBody';
import type { UpdateIntents, UpdateModel } from './model';

function useUpdateText(model: UpdateModel): { title: string; body: string | null } {
  const { t } = useTranslation();
  switch (model.status) {
    case 'checking':
      return { title: t('settings.update.checking'), body: null };
    case 'up_to_date':
      return { title: t('settings.more.upToDate'), body: null };
    case 'available':
      return { title: t('settings.update.title', { version: model.version }), body: model.notes };
    case 'downloading':
      return {
        title: t('settings.update.downloading', { version: model.version }),
        body: t('settings.update.downloadingBody'),
      };
    case 'verifying':
      return { title: t('settings.update.verifying'), body: t('settings.update.verifyingBody') };
    case 'waiting':
      return {
        title: t(`settings.update.waiting.${model.blocker}`),
        body: t('settings.update.waitingBody', { version: model.version }),
      };
    case 'installing':
      return { title: t('settings.update.installing', { version: model.version }), body: null };
    case 'installed':
      return {
        title: t('settings.update.installed'),
        body: t('settings.update.installedBody', { version: model.version }),
      };
    case 'failed':
      return errorText(model.error, t);
  }
}

/** The download bar and its numbers: percent, size, speed and time left, as measured. */
function DownloadBar({ progress }: { readonly progress: DownloadView | null }) {
  const { t } = useTranslation();
  const format = useFormat();
  const timeLeft = useTimeLeft();
  if (progress === null) return <Status>{t('settings.update.connecting')}</Status>;
  const size =
    progress.total === null || progress.percent === null
      ? t('settings.update.downloaded', { done: format.bytes(progress.done) })
      : t('settings.update.progress', {
          percent: progress.percent,
          done: format.bytes(progress.done),
          total: format.bytes(progress.total),
        });
  const speed =
    progress.bytes_per_sec === null
      ? null
      : t('transfer.rate', { size: format.bytes(progress.bytes_per_sec) });
  return (
    <Progress
      value={progress.percent === null ? 0 : progress.percent / 100}
      label={t('settings.update.bar')}
      meta={[
        speed === null ? size : `${size} · ${speed}`,
        progress.secs_left === null
          ? t('settings.update.noEstimate')
          : timeLeft(progress.secs_left),
      ]}
    />
  );
}

/** "Install anyway" asks first: installing can close Carrova mid-move or mid-call. */
function AnywayDialog({
  blocker,
  onWait,
  onInstall,
}: {
  readonly blocker: UpdateBlocker;
  readonly onWait: () => void;
  readonly onInstall: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      tone="warning"
      icon="warn"
      title={t(`settings.update.anyway.title.${blocker}`)}
      onDismiss={onWait}
      foot={
        <>
          <Button data-initial-focus onClick={onWait}>
            {t('settings.update.anyway.wait')}
          </Button>
          <Button kind="danger" onClick={onInstall}>
            {t('settings.update.anyway.confirm')}
          </Button>
        </>
      }
    >
      <p>{t(`settings.update.anyway.body.${blocker}`)}</p>
    </Dialog>
  );
}

/** The actions of each state: the safe one first (focused), the main one last. */
function UpdateFoot({
  model,
  on,
  onAnyway,
}: {
  readonly model: UpdateModel;
  readonly on: UpdateIntents;
  readonly onAnyway: () => void;
}) {
  const { t } = useTranslation();
  const cancel = (
    <Button kind="quiet" onClick={on.onCancel}>
      {t('action.cancel')}
    </Button>
  );
  const close = (label: string) => (
    <Button data-initial-focus onClick={on.onLater}>
      {label}
    </Button>
  );
  switch (model.status) {
    case 'available':
      return (
        <>
          {close(t('action.later'))}
          <Button kind="primary" onClick={on.onInstall}>
            {t('settings.update.install')}
          </Button>
        </>
      );
    case 'downloading':
    case 'verifying':
      return (
        <>
          {cancel}
          {close(t('ui.close'))}
        </>
      );
    case 'waiting':
      return (
        <>
          {cancel}
          <Button kind="quiet" onClick={onAnyway}>
            {t('settings.update.anyway.open')}
          </Button>
          {close(t('ui.close'))}
        </>
      );
    case 'installed':
      return (
        <>
          {close(t('action.later'))}
          <Button kind="primary" onClick={on.onRestart}>
            {t('settings.update.restart')}
          </Button>
        </>
      );
    case 'failed':
      return (
        <>
          {close(t('ui.close'))}
          {model.version !== null && (
            <Button kind="primary" onClick={on.onInstall}>
              {t('settings.update.again')}
            </Button>
          )}
        </>
      );
    case 'checking':
    case 'up_to_date':
    case 'installing':
      return close(t('ui.close'));
  }
}

/**
 * App updates: offered, installed only on the user's click (live download, signature check,
 * waiting for a running move or call), then a restart they choose.
 */
export function UpdateDialog({
  model,
  on,
}: {
  readonly model: UpdateModel;
  readonly on: UpdateIntents;
}) {
  const text = useUpdateText(model);
  const { t } = useTranslation();
  const [asking, setAsking] = useState(false);
  if (asking && model.status === 'waiting')
    return (
      <AnywayDialog
        blocker={model.blocker}
        onWait={() => {
          setAsking(false);
        }}
        onInstall={() => {
          setAsking(false);
          on.onInstallAnyway();
        }}
      />
    );
  return (
    <Dialog
      icon={model.status === 'failed' ? 'warn' : 'receive'}
      title={text.title}
      onDismiss={on.onLater}
      foot={
        <UpdateFoot
          model={model}
          on={on}
          onAnyway={() => {
            setAsking(true);
          }}
        />
      }
    >
      {text.body !== null && <p>{text.body}</p>}
      {model.status === 'downloading' && <DownloadBar progress={model.progress} />}
      {(model.status === 'checking' ||
        model.status === 'verifying' ||
        model.status === 'installing') && <Status>{t('settings.update.wait')}</Status>}
      {(model.status === 'waiting' || model.status === 'installed') && (
        <LockLine>{t('settings.update.verified')}</LockLine>
      )}
    </Dialog>
  );
}
