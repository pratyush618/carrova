import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { cx } from '../../ui/cx';
import { Icon, useReducedMotion } from '../../ui';
import type { DeviceEntry } from './model';
import { place, type Placement } from './place';
import { MIN_FIELD, MORE, RING_ORDER, SLOTS, spot, type Slot } from './spots';

/** A map spot's CSS offset from the centre (`--k` scales it by ring). */
const offset = (slot: Slot) => {
  const { dx, dy } = spot(slot);
  return { '--dx': String(dx), '--dy': String(dy) };
};

const sameSeats = (a: readonly Placement[], b: readonly Placement[]) =>
  a.length === b.length &&
  a.every((p) => b.some((q) => q.key === p.key && q.ring === p.ring && q.slot === p.slot));

/** The field's width, followed as the window resizes (`null` until measured). */
function useFieldWidth() {
  const field = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState<number | null>(null);
  // Measured before the first paint, so a wide field never shows the narrow layout first.
  useLayoutEffect(() => {
    const element = field.current;
    if (element === null) return;
    // 0: not laid out (yet), not a width.
    const now = element.getBoundingClientRect().width;
    if (now > 0) setWidth(now);
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) setWidth(entry.contentRect.width);
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, []);
  return { field, width };
}

function DeviceDot({
  device,
  slot,
  selected,
  onSelect,
}: {
  readonly device: DeviceEntry;
  readonly slot: Slot;
  readonly selected: boolean;
  readonly onSelect: () => void;
}) {
  const { t } = useTranslation();
  return (
    <li className={`s-near__spot s-near__spot--${device.ring}`} style={offset(slot)}>
      <button
        type="button"
        data-key={device.key}
        title={device.name}
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
        <span className="s-near__dev-text">
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
 * Devices keep their slots while they stay; those with no room are summed
 * up in a "+N more" chip that opens the list (`onMore`).
 */
export function NearbyMap({
  devices,
  selected,
  pulse,
  onPulsed,
  onSelect,
  onMore,
  seats,
  onSeats,
  children,
}: {
  readonly devices: readonly DeviceEntry[];
  readonly selected: string | null;
  readonly pulse: boolean;
  readonly onPulsed: () => void;
  readonly onSelect: (key: string | null) => void;
  readonly onMore: () => void;
  /** The slots devices last had, kept by the page so they survive a trip to the list. */
  readonly seats: readonly Placement[];
  readonly onSeats: (seats: readonly Placement[]) => void;
  /** Over the field: the selected device's card, or the "looking" note. */
  readonly children?: ReactNode;
}) {
  const { t } = useTranslation();
  const reduced = useReducedMotion();
  const { field, width } = useFieldWidth();
  const { placed, more } = place(devices, seats, width ?? MIN_FIELD, selected);
  // Seats chosen before the field is measured are not kept, or a wide field would stay laid out as a narrow one.
  const settled = width === null || sameSeats(placed, seats);
  useEffect(() => {
    if (!settled) onSeats(placed);
  });
  const shownInOrder = placed.toSorted(
    (a, b) => RING_ORDER.indexOf(a.ring) - RING_ORDER.indexOf(b.ring) || a.slot - b.slot,
  );
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
    <div ref={field} className="s-near__field">
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
        {shownInOrder.map(({ key, ring, slot }) => {
          const device = devices.find((d) => d.key === key);
          const where = SLOTS[ring][slot];
          return (
            device &&
            where && (
              <DeviceDot
                key={key}
                device={device}
                slot={where}
                selected={key === selected}
                onSelect={() => {
                  onSelect(key === selected ? null : key);
                }}
              />
            )
          );
        })}
        {more.length > 0 && (
          <li className="s-near__spot" style={offset(MORE)}>
            <button
              type="button"
              className="s-near__more"
              aria-label={t('nearby.moreAll', { count: devices.length, more: more.length })}
              onClick={onMore}
            >
              {t('nearby.more', { count: more.length })}
            </button>
          </li>
        )}
      </ul>
      {children}
    </div>
  );
}
