import { useReducedMotion } from '../motion';
import './Arrival.css';

const PARTICLES = 14;

/** Burst particle vectors: an ellipse of 14 pieces at three radii (deterministic). */
const BURST = Array.from({ length: PARTICLES }, (_, i) => {
  const angle = (i / PARTICLES) * Math.PI * 2;
  const radius = 90 + (i % 3) * 30;
  return {
    '--dx': `${(Math.cos(angle) * radius).toFixed(1)}px`,
    '--dy': `${(Math.sin(angle) * radius * 0.6).toFixed(1)}px`,
    '--rot': `${String((i * 47) % 360)}deg`,
  };
});

export interface ArrivalProps {
  /** The stamp word ("Arrived", "Delivered", "Packed"). */
  readonly word: string;
  /** One line under it ("5 apps, 1.2 GB"). */
  readonly meta: string;
}

/** The completion moment: a stamp lands with a small burst (static under reduced motion). */
export function Arrival({ word, meta }: ArrivalProps) {
  const reduced = useReducedMotion();
  return (
    <div className="p-arrival">
      <div className="p-stamp" role="img" aria-label={`${word}. ${meta}`}>
        <span className="p-stamp__word">{word}</span>
        <span className="p-stamp__meta">{meta}</span>
      </div>
      {!reduced && (
        <div className="p-burst" aria-hidden="true" data-testid="arrival-burst">
          {BURST.map((style, i) => (
            <i key={i} style={style} />
          ))}
        </div>
      )}
    </div>
  );
}
