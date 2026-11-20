import type { FeatureRowView, LinkedDeviceView } from '../../engine/types';
import { useTranslation } from 'react-i18next';
import { Banner, Button, Head, HubLayout, LinkButton, Row, Toggle } from '../../ui';
import { CallsLinks } from '../phone/calls/CallsLinks';
import { Group } from '../settings/Group';
import { FeatureRow } from './FeatureRow';
import { NEEDS_ACCESS, isPhase2, type DeviceIntents, type Phase2Feature } from './model';
import './devices.css';

type Phase2Row = FeatureRowView & { readonly feature: Phase2Feature };

const isPhase2Row = (row: FeatureRowView): row is Phase2Row => isPhase2(row.feature);

/** What flows for `feature` now (off when the link doesn't carry it). */
const flows = (device: LinkedDeviceView, feature: Phase2Feature): boolean =>
  device.features.some((f) => f.feature === feature && f.effective !== 'off');

/** One linked device's page in the desktop's hub. */
export function DevicePage(props: DeviceDetailsProps) {
  return (
    <HubLayout>
      <DeviceDetails {...props} />
    </HubLayout>
  );
}

export interface DeviceDetailsProps {
  readonly device: LinkedDeviceView;
  /** The engine refused the last feature change. */
  readonly failed?: boolean;
  /** This computer rang it and the ring still runs. */
  readonly ringing?: boolean;
  /** Moves with this computer, no code (`peer:linked`, `peer:noreq`); absent for phones, or while that is off. */
  readonly move?: DeviceMove;
  readonly on: DeviceIntents;
}

/** A trusted computer's "Move your setup" group. */
export interface DeviceMove {
  /** It may send this computer move requests. */
  readonly sendRequests: boolean;
  readonly onSendSetup: () => void;
  readonly onSendRequests: (on: boolean) => void;
}

/** Send it a move without a code, and whether it may send you one. */
function MoveGroup({
  move,
  name,
  here,
}: {
  readonly move: DeviceMove;
  readonly name: string;
  readonly here: boolean;
}) {
  const { t } = useTranslation();
  return (
    <Group title={t('request.device.group')}>
      <Row
        off={!here}
        title={t('request.device.what')}
        sub={t('request.device.whatSub')}
        trail={
          <Button size="sm" kind="primary" icon="send" disabled={!here} onClick={move.onSendSetup}>
            {t('request.device.sendSetup')}
          </Button>
        }
      />
      <Row
        title={t('request.device.requests')}
        sub={t(move.sendRequests ? 'request.device.requestsOn' : 'request.device.requestsOff', {
          name,
        })}
        trail={
          <Toggle
            on={move.sendRequests}
            label={t('request.device.requests')}
            onChange={move.onSendRequests}
          />
        }
      />
    </Group>
  );
}

/**
 * One linked device: what it can do with this device, in two groups
 * (things that run quietly once on, things that ask every time), and the
 * few actions it offers now. Laid out by the page around it (desktop hub, phone).
 */
export function DeviceDetails({
  device,
  failed = false,
  ringing = false,
  move,
  on,
}: DeviceDetailsProps) {
  const { t } = useTranslation();
  const here = device.state === 'linked';
  const rows = device.features.filter(isPhase2Row);
  const missing = NEEDS_ACCESS.filter((f) => here && device.missing.includes(f));
  const { onApps } = on;
  const group = (kind: FeatureRowView['group']) =>
    rows
      .filter((row) => row.group === kind)
      .map((row) => (
        <FeatureRow
          key={row.feature}
          feature={row.feature}
          row={row}
          device={device.name}
          off={!here}
          waiting={missing.some((f) => f === row.feature)}
          {...(row.feature === 'notifications' &&
            onApps &&
            here &&
            row.effective !== 'off' &&
            !missing.includes('notifications') && {
              action: (
                <Button size="sm" kind="quiet" onClick={onApps}>
                  {t('ecosystem.device.apps')}
                </Button>
              ),
            })}
          onChange={(state) => {
            on.onFeature(row.feature, state);
          }}
        />
      ));
  const { onCalls, onCallAudio } = on;
  // Calls is the last quiet row: its links go right under it.
  const callsOn = rows.some((row) => row.feature === 'calls' && row.local === 'on');
  const quiet = [
    ...group('quiet'),
    ...(callsOn && onCalls
      ? [<CallsLinks key="calls-links" onCalls={onCalls} onCallAudio={onCallAudio} />]
      : []),
  ];
  const ask = group('ask');
  return (
    <div className="s-device">
      <Head
        title={device.name}
        lede={`${t(`ecosystem.status.${device.state}`)} · ${t(`settings.about.os.${device.os}`)}`}
      />
      {here && (
        <div className="s-device__acts">
          {device.os === 'android' && flows(device, 'ring') && (
            <Button size="sm" icon="ring" onClick={ringing ? on.onStopRing : on.onRing}>
              {t(ringing ? 'ecosystem.device.stopRing' : 'ecosystem.device.ring')}
            </Button>
          )}
          {flows(device, 'clipboard') && (
            <Button size="sm" icon="copy" onClick={on.onSendClipboard}>
              {t('ecosystem.device.sendClipboard')}
            </Button>
          )}
          {on.onSendFiles && flows(device, 'files') && (
            <Button size="sm" icon="file" onClick={on.onSendFiles}>
              {t('ecosystem.device.sendFiles')}
            </Button>
          )}
        </div>
      )}
      {device.state === 'away' && (
        <Banner
          tone="accent"
          title={t('ecosystem.device.awayTitle', { name: device.name })}
          body={t('ecosystem.device.awayBody')}
        />
      )}
      {missing.map((feature) => (
        <Banner
          key={feature}
          tone="warning"
          title={t(`ecosystem.device.missing.${feature}.title`, { name: device.name })}
          body={t(`ecosystem.device.missing.${feature}.body`)}
          {...(on.onRemind && {
            action: (
              <Button
                size="sm"
                onClick={() => {
                  on.onRemind?.(feature);
                }}
              >
                {t('ecosystem.device.remind')}
              </Button>
            ),
          })}
        />
      ))}
      {failed && <Banner tone="warning" title={t('ecosystem.device.failed')} />}
      {quiet.length > 0 && <Group title={t('ecosystem.groups.quiet')}>{quiet}</Group>}
      {ask.length > 0 && (
        <Group title={t('ecosystem.groups.ask')} sub={t('ecosystem.askFoot')}>
          {ask}
        </Group>
      )}
      {move && <MoveGroup move={move} name={device.name} here={here} />}
      <p className="s-device__foot">
        <span className="p-mono">{t('ecosystem.device.key', { key: device.fingerprint })}</span>
        <LinkButton onClick={on.onForget}>
          {t('ecosystem.device.forget', { name: device.name })}
        </LinkButton>
      </p>
    </div>
  );
}
