import { useId } from 'react';
import { useReducedMotion } from '../motion';
import './Seal.css';

const PIECES = 16;

/** Burst vectors: 16 pieces on a circle at three radii (deterministic). */
const BURST = Array.from({ length: PIECES }, (_, i) => {
  const angle = (i / PIECES) * Math.PI * 2;
  const radius = 90 + (i % 3) * 22;
  return {
    '--dx': `${(Math.cos(angle) * radius).toFixed(1)}px`,
    '--dy': `${(Math.sin(angle) * radius).toFixed(1)}px`,
    '--rot': `${String(i * 47)}deg`,
  };
});

export interface SealProps {
  /** Words that run around the ring, already repeated to fill it ("ARRIVED · ALL CHECKED · …"). */
  readonly ring: string;
  /** What the seal means, for assistive tech ("Everything arrived and was checked"). */
  readonly label: string;
}

/**
 * The completion moment: a seal stamps in, its tick draws, and a small burst
 * plays once. Under reduced motion it is simply there, without the burst.
 */
export function Seal({ ring, label }: SealProps) {
  const reduced = useReducedMotion();
  const circle = useId();
  return (
    <div className="p-seal" role="img" aria-label={label}>
      <svg viewBox="0 0 140 140" aria-hidden="true">
        <defs>
          <path id={circle} d="M70 70m-52 0a52 52 0 1 1 104 0a52 52 0 1 1-104 0" />
        </defs>
        <circle className="p-seal__core" cx="70" cy="70" r="64" />
        <circle className="p-seal__ring" cx="70" cy="70" r="61" />
        <circle className="p-seal__ring" cx="70" cy="70" r="41" />
        <g className="p-seal__spin">
          <text>
            <textPath href={`#${circle}`} textLength="322" lengthAdjust="spacing">
              {ring}
            </textPath>
          </text>
        </g>
        <path className="p-seal__tick" d="M56 71 66 81 85 60" />
      </svg>
      {!reduced && (
        <span className="p-seal__burst" aria-hidden="true" data-testid="seal-burst">
          {BURST.map((style, i) => (
            <i key={i} style={style} />
          ))}
        </span>
      )}
    </div>
  );
}
