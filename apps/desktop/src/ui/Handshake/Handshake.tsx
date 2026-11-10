import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import './Handshake.css';

export interface HandshakeProps {
  /** This computer's name. */
  readonly here: string;
  /** The other computer's name. */
  readonly there: string;
  /** The lock clicks in once the code is verified. */
  readonly linked: boolean;
}

function End({ name }: { readonly name: string }) {
  return (
    <div className="p-handshake__end">
      <span className="p-handshake__pc">
        <Icon name="laptop" size="lg" />
      </span>
      {name}
    </div>
  );
}

/**
 * Two computers, one encrypted link: the signature moment of pairing.
 * Decorative; the adjacent status line says the same thing in words.
 */
export function Handshake({ here, there, linked }: HandshakeProps) {
  return (
    <div className={cx('p-handshake', linked && 'p-handshake--linked')} aria-hidden="true">
      <End name={here} />
      <div className="p-handshake__wire">
        {linked ? (
          <span className="p-handshake__lock">
            <Icon name="lock" />
          </span>
        ) : (
          <>
            <span className="p-handshake__dot" />
            <span className="p-handshake__dot" />
          </>
        )}
      </div>
      <End name={there} />
    </div>
  );
}
