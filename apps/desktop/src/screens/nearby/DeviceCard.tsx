import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { AppGlyph, Badge, Button, IconButton } from '../../ui';
import { deviceActions, type DeviceAction, type DeviceEntry, type NearbyIntents } from './model';

/** How each action looks: Send leads, Wake is the way back for an away device, Forget stays quiet. */
const LOOK: Readonly<Record<DeviceAction, 'primary' | 'secondary' | 'quiet'>> = {
  sendWithCode: 'secondary',
  wake: 'secondary',
  forget: 'quiet',
};

/** The buttons `device` offers, each naming the device for assistive tech. */
export function DeviceActions({
  device,
  on,
}: {
  readonly device: DeviceEntry;
  readonly on: NearbyIntents;
}) {
  const { t } = useTranslation();
  const run: Readonly<Record<DeviceAction, () => void>> = {
    sendWithCode: on.onSendWithCode,
    wake: () => {
      on.onWake(device);
    },
    forget: () => {
      on.onForget(device);
    },
  };
  return (
    <>
      {deviceActions(device).map((action) => {
        const label = t(`nearby.actions.${action}`);
        return (
          <Button
            key={action}
            kind={LOOK[action]}
            size="sm"
            aria-label={t('nearby.actionFor', { action: label, name: device.name })}
            onClick={run[action]}
          >
            {label}
          </Button>
        );
      })}
    </>
  );
}

/** "Linux, last move yesterday": the device's system and when you last moved with it (a new one has its badge). */
export function useDeviceSub(): (device: DeviceEntry, now: Date) => string {
  const { t } = useTranslation();
  const format = useFormat();
  return (device, now) =>
    [
      device.os === null ? null : t(`settings.about.os.${device.os}`),
      device.kind === 'new'
        ? null
        : device.lastMove === null
          ? t('nearby.noMove')
          : t('nearby.lastMove', { day: format.day(device.lastMove, now) }),
    ]
      .filter((part) => part !== null)
      .join(', ');
}

/** The selected device on the map: what you have done together, and what you can do now. */
export function DeviceCard({
  device,
  now,
  on,
}: {
  readonly device: DeviceEntry;
  readonly now: Date;
  readonly on: NearbyIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <section className="s-near__card" aria-label={device.name}>
      <header className="s-near__card-head">
        <AppGlyph
          glyph={{ icon: 'laptop' }}
          tone={device.kind === 'known' ? 'accent' : 'neutral'}
        />
        <div>
          <h2>
            {device.name}
            {device.kind === 'new' && <Badge tone="warning">{t('nearby.new')}</Badge>}
          </h2>
          {device.os !== null && <span>{t(`settings.about.os.${device.os}`)}</span>}
        </div>
        <IconButton
          icon="close"
          label={t('ui.close')}
          onClick={() => {
            on.onSelect(null);
          }}
        />
      </header>
      {device.fingerprint !== null && (
        <dl className="s-near__facts">
          <div>
            <dt>{t('nearby.card.lastMove')}</dt>
            <dd>
              {device.lastMove === null
                ? t('nearby.card.never')
                : format.day(device.lastMove, now, true)}
            </dd>
          </div>
          <div>
            <dt>{t('nearby.card.moves')}</dt>
            <dd>{format.number(device.moves)}</dd>
          </div>
          <div>
            <dt>{t('nearby.card.key')}</dt>
            <dd className="p-mono">{device.fingerprint}</dd>
          </div>
        </dl>
      )}
      <div className="s-near__card-acts">
        <DeviceActions device={device} on={on} />
      </div>
    </section>
  );
}
