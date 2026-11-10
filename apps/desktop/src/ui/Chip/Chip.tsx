import type { ReactNode } from 'react';
import { AppGlyph } from '../AppGlyph/AppGlyph';
import { Icon } from '../Icon/Icon';
import './Chip.css';

export type ChipState = 'waiting' | 'moving' | 'done' | 'failed';

export interface ChipProps {
  readonly glyph: string;
  readonly name: string;
  readonly sub: ReactNode;
  readonly state: ChipState;
  /** What is happening to this app now ("Arrived", "612 MB of 1.2 GB"). */
  readonly stateText: ReactNode;
}

/** One app moving. Done chips swap the glyph for a tick that springs in. */
export function Chip({ glyph, name, sub, state, stateText }: ChipProps) {
  return (
    <li className="p-chip" data-state={state}>
      <AppGlyph glyph={glyph} done={state === 'done'} />
      <span>
        <span className="p-row__title">{name}</span>
        <span className="p-row__sub">{sub}</span>
      </span>
      <span className="p-chip__state">
        {state === 'done' && <Icon name="check" size="sm" />}
        {stateText}
      </span>
    </li>
  );
}

/** What is moving, small configs first (the engine's real order). */
export function Chips({
  label,
  children,
}: {
  readonly label: string;
  readonly children: ReactNode;
}) {
  return (
    <ul className="p-chips" aria-label={label}>
      {children}
    </ul>
  );
}
