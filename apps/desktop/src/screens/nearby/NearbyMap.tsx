import { useEffect, useRef, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Ring } from '../../engine/types';
import { cx } from '../../ui/cx';
import { Icon, useReducedMotion } from '../../ui';
import type { DeviceEntry } from './model';
import { RING_ORDER, spot } from './spots';

function DeviceDot({
  device,
  at,
  selected,
  onSelect,
}: {
  readonly device: DeviceEntry;
  readonly at: { dx: number; dy: number };
  readonly selected: boolean;
  readonly onSelect: () => void;
}) {
  const { t } = useTranslation();
  return (
    <li
      className={`s-near__spot s-near__spot--${device.ring}`}
      style={{ '--dx': String(at.dx), '--dy': String(at.dy) }}
    >
      <button
        type="button"
        data-key={device.key}
        className={cx(
          's-near__dev',
          `s-near__dev--${device.kind}`,
          selected && 's-near__dev--selected',
        )}
        aria-pressed={selected}
        onClick={onSelect}
      >
        <span className="s-near__dev-icon">
          <Icon name="laptop" />
        </span>
        <span>
          <b>{device.name}</b>
          <small>{t(`nearby.status.${device.kind}`)}</small>
        </span>
      </button>
    </li>
  );
}

/**
 * This computer in the middle, devices on rings by how fast they answered
 * (not distance), away ones faded outside. One ring of light leaves this
 * computer when the page opens (`pulse`), never under reduced motion.
 */
export function NearbyMap({
  devices,
  selected,
  pulse,
  onPulsed,
  onSelect,
  children,
}: {
  readonly devices: readonly DeviceEntry[];
  readonly selected: string | null;
  readonly pulse: boolean;
  readonly onPulsed: () => void;
  readonly onSelect: (key: string | null) => void;
  /** Over the field: the selected device's card, or the "looking" note. */
  readonly children?: ReactNode;
}) {
  const { t } = useTranslation();
  const reduced = useReducedMotion();
  const byRing = (ring: Ring) => devices.filter((device) => device.ring === ring);
  // Closing a card hands the keyboard back to its device, unless it already went elsewhere.
  const list = useRef<HTMLUListElement>(null);
  const shown = useRef(selected);
  useEffect(() => {
    const was = shown.current;
    shown.current = selected;
    if (selected !== null || was === null || document.activeElement !== document.body) return;
    const buttons = list.current?.querySelectorAll<HTMLButtonElement>('button[data-key]') ?? [];
    [...buttons].find((button) => button.dataset['key'] === was)?.focus();
  }, [selected]);
  return (
    <div className="s-near__field">
      <div className="s-near__orbs" aria-hidden="true">
        <i />
        <i />
        <i />
        <i />
      </div>
      {pulse && !reduced && (
        <div className="s-near__pulse" aria-hidden="true" onAnimationEnd={onPulsed} />
      )}
      <div className="s-near__me">
        <span className="s-near__me-disc" aria-hidden="true">
          <Icon name="laptop" />
        </span>
        <b>{t('nearby.me')}</b>
      </div>
      <ul ref={list} className="s-near__devs" aria-label={t('nearby.map')}>
        {RING_ORDER.flatMap((ring) => {
          const on = byRing(ring);
          return on.map((device, i) => (
            <DeviceDot
              key={device.key}
              device={device}
              at={spot(ring, i, on.length)}
              selected={device.key === selected}
              onSelect={() => {
                onSelect(device.key === selected ? null : device.key);
              }}
            />
          ));
        })}
      </ul>
      {children}
    </div>
  );
}
