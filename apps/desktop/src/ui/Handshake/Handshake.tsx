import type { OsKind } from '../../bindings/OsKind';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import { OsBadge } from '../OsBadge/OsBadge';
import './Handshake.css';

export interface HandshakeProps {
  /** This computer's name. */
  readonly here: string;
  /** The other computer's name. */
  readonly there: string;
  /** The code is proven: the wire draws and the lock clicks shut. Before, a dot shuttles and the lock is open. */
  readonly linked: boolean;
  /** Each computer's OS, once known: shown as a badge under its name. */
  readonly hereOs?: HandshakeOs | null;
  readonly thereOs?: HandshakeOs | null;
}

/** An OS family and what to call it ("macOS"). */
export interface HandshakeOs {
  readonly kind: OsKind;
  readonly label: string;
}

function Device({ name, os }: { readonly name: string; readonly os: HandshakeOs | null }) {
  return (
    <div className="p-handshake__device">
      <span className="p-handshake__screen">
        <Icon name="laptop" size="lg" />
      </span>
      <span className="p-handshake__name">{name}</span>
      {os !== null && <OsBadge os={os.kind} label={os.label} size="sm" />}
    </div>
  );
}

/**
 * Two computers slide together, the wire draws and the lock clicks shut: the
 * signature moment of pairing. Decorative (the heading says the same in
 * words); under reduced motion it simply shows the end state.
 */
export function Handshake({ here, there, linked, hereOs = null, thereOs = null }: HandshakeProps) {
  return (
    <div
      className={cx('p-handshake', linked ? 'p-handshake--linked' : 'p-handshake--pending')}
      aria-hidden="true"
    >
      <Device name={here} os={hereOs} />
      <div className="p-handshake__wire">
        <svg className="p-handshake__line" viewBox="0 0 200 4" preserveAspectRatio="none">
          <path d="M0 2H200" />
          <path className="p-handshake__live" d="M0 2H200" />
        </svg>
        {!linked && <span className="p-handshake__dot" />}
        <span className="p-handshake__lock">
          <svg
            width="26"
            height="26"
            viewBox="0 0 26 26"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
          >
            <rect x="6" y="11.5" width="14" height="10" rx="2.5" />
            <path className="p-handshake__shackle" d="M9 11.5V8.5a4 4 0 0 1 8 0v3" />
          </svg>
        </span>
      </div>
      <Device name={there} os={thereOs} />
    </div>
  );
}
