import { Fragment, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { cx } from '../cx';
import { useReducedMotion } from '../motion';
import { FLAP_TICK_MS, flapFace, revealDuration, settleAt } from './flaps';
import './CodeDisplay.css';

export interface CodeDisplayProps {
  /** Code words in order, e.g. `['7', 'zebra', 'lemon']`. */
  readonly words: readonly string[];
  /** Flip the tiles in like a departure board (static under reduced motion). */
  readonly reveal?: boolean;
  readonly size?: 'md' | 'sm';
}

/** Milliseconds since mount; stops ticking once every tile has settled. */
function useRevealClock(active: boolean, duration: number): number {
  const [elapsed, setElapsed] = useState(0);
  useEffect(() => {
    if (!active) return undefined;
    const start = performance.now();
    const timer = window.setInterval(() => {
      const now = performance.now() - start;
      setElapsed(now);
      if (now >= duration) window.clearInterval(timer);
    }, FLAP_TICK_MS);
    return () => {
      window.clearInterval(timer);
    };
  }, [active, duration]);
  return active ? elapsed : Number.POSITIVE_INFINITY;
}

/** The pairing code as split-flap tiles: the one bold element of the send flow. */
export function CodeDisplay(props: CodeDisplayProps) {
  // A new code restarts the reveal from scratch.
  return <CodeTiles key={props.words.join('-')} {...props} />;
}

function CodeTiles({ words, reveal = false, size = 'md' }: CodeDisplayProps) {
  const { t } = useTranslation();
  const reduced = useReducedMotion();
  const offsets = words.map((_, w) => words.slice(0, w).join('').length);
  const elapsed = useRevealClock(reveal && !reduced, revealDuration(words.join('').length));

  return (
    <div className="p-code-fit">
      <div
        className={cx('p-code', size === 'sm' && 'p-code--sm')}
        role="img"
        aria-label={t('ui.code.label', { code: words.join(' ') })}
      >
        {words.map((word, w) => (
          <Fragment key={`${String(w)}-${word}`}>
            {w > 0 && <span className="p-code__dash" />}
            <span className="p-code__word" aria-hidden="true">
              {Array.from(word, (ch, c) => {
                const i = (offsets[w] ?? 0) + c;
                const set = elapsed >= settleAt(i);
                return (
                  <span
                    key={i}
                    className={cx(
                      'p-flap',
                      /\d/.test(ch) && 'p-flap--num',
                      reveal && (set ? 'p-flap--set' : 'p-flap--spinning'),
                    )}
                    data-settled={set}
                  >
                    {flapFace(ch, i, elapsed)}
                  </span>
                );
              })}
            </span>
          </Fragment>
        ))}
      </div>
    </div>
  );
}
