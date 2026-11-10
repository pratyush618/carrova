import type { ReactNode } from 'react';
import './Window.css';

export interface WindowProps {
  /** The screen (a hub, flow or bare layout). */
  readonly children: ReactNode;
  /** Open sheets and dialogs, bottom to top. Only the top one is interactive. */
  readonly overlays?: readonly ReactNode[];
}

/**
 * The app window. Overlays sit inside it; everything under the top overlay
 * is `inert`, so focus and assistive tech stay in the overlay.
 */
export function Window({ children, overlays = [] }: WindowProps) {
  const top = overlays.length - 1;
  return (
    <div className="p-window">
      <div className="p-window__base" inert={overlays.length > 0}>
        {children}
      </div>
      {overlays.map((overlay, i) => (
        <div key={i} inert={i < top}>
          {overlay}
        </div>
      ))}
    </div>
  );
}
