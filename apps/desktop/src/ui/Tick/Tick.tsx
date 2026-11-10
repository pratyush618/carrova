import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import './Tick.css';

/** Where one app is in a transfer. */
export type TickState = 'queued' | 'now' | 'done' | 'wait';

/** The status mark at the start of a transfer row. Decorative: the row's text says the state. */
export function Tick({ state }: { readonly state: TickState }) {
  return (
    <span className={cx('p-tick', `p-tick--${state}`)} aria-hidden="true">
      {state === 'done' && <Icon name="check" size="sm" />}
      {state === 'wait' && <Icon name="pause" size="sm" />}
    </span>
  );
}
