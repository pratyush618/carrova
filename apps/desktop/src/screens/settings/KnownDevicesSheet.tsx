import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Button, Dialog, IconButton, Input, List, Note, OsBadge, Row, Sheet } from '../../ui';
import type { DeviceChange } from '../nearby/model';
import type { KnownDeviceLine, KnownDevicesIntents } from './model';

/** One remembered device: name, system, last move and key; renamed in place. */
function DeviceRow({
  device,
  now,
  on,
}: {
  readonly device: KnownDeviceLine;
  readonly now: Date;
  readonly on: KnownDevicesIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const [draft, setDraft] = useState<string | null>(null);
  const save = (event: { preventDefault: () => void }) => {
    event.preventDefault();
    if (draft !== null && draft.trim() !== '' && draft.trim() !== device.name)
      on.onRename(device.id, draft.trim());
    setDraft(null);
  };
  const title =
    draft === null ? (
      <>
        {device.name}{' '}
        <OsBadge os={device.os} label={t(`settings.about.os.${device.os}`)} size="sm" />
      </>
    ) : (
      <form className="s-devices__rename" onSubmit={save}>
        <Input
          aria-label={t('settings.devices.name', { name: device.name })}
          value={draft}
          autoFocus
          onChange={(event) => {
            setDraft(event.target.value);
          }}
        />
        <Button size="sm" onClick={save}>
          {t('settings.devices.save')}
        </Button>
      </form>
    );
  return (
    <Row
      title={title}
      sub={[
        device.lastMove === null
          ? t('settings.devices.noMove')
          : t('settings.devices.lastMove', { day: format.day(device.lastMove, now) }),
        t('settings.devices.key', { fingerprint: device.fingerprint }),
      ].join(' · ')}
      trail={
        <>
          {draft === null && (
            <IconButton
              icon="pen"
              label={t('settings.devices.rename', { name: device.name })}
              onClick={() => {
                setDraft(device.name);
              }}
            />
          )}
          <Button
            kind="quiet"
            size="sm"
            aria-label={t('settings.devices.forgetOne', { name: device.name })}
            onClick={() => {
              on.onForget(device.id);
            }}
          >
            {t('settings.devices.forget')}
          </Button>
        </>
      }
    />
  );
}

/** Remembered devices: rename or forget each, or forget them all. */
export function KnownDevicesSheet({
  devices,
  failed = null,
  now,
  on,
}: {
  readonly devices: readonly KnownDeviceLine[];
  /** The last change the engine refused. */
  readonly failed?: DeviceChange | null;
  readonly now: Date;
  readonly on: KnownDevicesIntents;
}) {
  const { t } = useTranslation();
  return (
    <Sheet
      title={t('settings.devices.title')}
      sub={t('settings.devices.sub', { count: devices.length })}
      onClose={on.onDone}
      foot={
        <>
          {devices.length > 0 && (
            <Button kind="danger" onClick={on.onForgetAll}>
              {t('settings.devices.forgetAll')}
            </Button>
          )}
          <span className="s-devices__spacer" />
          <Button kind="primary" onClick={on.onDone}>
            {t('action.done')}
          </Button>
        </>
      }
    >
      {devices.length === 0 ? (
        <Note>{t('settings.devices.empty')}</Note>
      ) : (
        <List label={t('settings.devices.title')}>
          {devices.map((device) => (
            <DeviceRow key={device.id} device={device} now={now} on={on} />
          ))}
        </List>
      )}
      {failed !== null && (
        <p role="status" className="s-settings-note">
          {t(`nearby.failed.${failed}`)}
        </p>
      )}
      <Note>{t('settings.devices.why')}</Note>
    </Sheet>
  );
}

/** Forgetting every device, or a new identity for this computer, is asked once, plainly. */
export function ForgetDevicesDialog({
  what,
  on,
}: {
  readonly what: 'all' | 'identity';
  readonly on: { readonly onCancel: () => void; readonly onConfirm: () => void };
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      tone="warning"
      icon="warn"
      title={t(`settings.forgetDevices.${what}.title`)}
      onDismiss={on.onCancel}
      foot={
        <>
          <Button kind="primary" data-initial-focus onClick={on.onCancel}>
            {t('action.cancel')}
          </Button>
          <Button kind="danger" onClick={on.onConfirm}>
            {t(`settings.forgetDevices.${what}.confirm`)}
          </Button>
        </>
      }
    >
      <p>{t(`settings.forgetDevices.${what}.body`)}</p>
    </Dialog>
  );
}
