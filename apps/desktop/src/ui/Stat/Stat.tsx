import type { ReactNode } from 'react';
import './Stat.css';

export interface StatProps {
  readonly value: ReactNode;
  readonly label: ReactNode;
}

/** A number with a plain label. */
export function Stat({ value, label }: StatProps) {
  return (
    <div className="p-stat">
      <div className="p-stat__n">{value}</div>
      <div className="p-stat__l">{label}</div>
    </div>
  );
}

/** A row of {@link Stat}s. */
export function Stats({ children }: { readonly children: ReactNode }) {
  return <div className="p-stats">{children}</div>;
}
