import { useEffect, useRef, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { AppGlyph, Badge, List, Row } from '../../ui';
import { DeviceActions, useDeviceSub } from './DeviceCard';
import type { DeviceEntry, NearbyIntents } from './model';

function DeviceRows({
  title,
  devices,
  now,
  on,
}: {
  readonly title: string;
  readonly devices: readonly DeviceEntry[];
  readonly now: Date;
  readonly on: NearbyIntents;
}) {
  const { t } = useTranslation();
  const sub = useDeviceSub();
  if (devices.length === 0) return null;
  return (
    <section className="s-near__group">
      <h2 tabIndex={-1}>{title}</h2>
      <List label={title}>
        {devices.map((device) => (
          <Row
            key={device.key}
            off={device.kind === 'away'}
            lead={
              <AppGlyph
                glyph={{ icon: 'laptop' }}
                tone={device.kind === 'known' ? 'accent' : 'neutral'}
              />
            }
            title={
              <>
                {device.name}
                {device.kind === 'new' && <Badge tone="warning">{t('nearby.new')}</Badge>}
              </>
            }
            sub={sub(device, now)}
            trail={<DeviceActions device={device} on={on} />}
          />
        ))}
      </List>
    </section>
  );
}

/** The same devices as the map, as rows: here now first, then the known ones away (faded). */
export function NearbyList({
  here,
  away,
  now,
  on,
  focusOnOpen = false,
  children,
}: {
  readonly here: readonly DeviceEntry[];
  readonly away: readonly DeviceEntry[];
  readonly now: Date;
  readonly on: NearbyIntents;
  /** Opened from the map's chip: the keyboard moves to the first heading. */
  readonly focusOnOpen?: boolean;
  /** Above the rows: the "looking" note while nobody answers. */
  readonly children?: ReactNode;
}) {
  const { t } = useTranslation();
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focusOnOpen) root.current?.querySelector('h2')?.focus();
  }, [focusOnOpen]);
  return (
    <div className="s-near__list" ref={root}>
      {children}
      <DeviceRows title={t('nearby.here')} devices={here} now={now} on={on} />
      <DeviceRows title={t('nearby.away')} devices={away} now={now} on={on} />
    </div>
  );
}
