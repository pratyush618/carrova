import { createContext, useState, type ReactNode } from 'react';
import './Window.css';

/** Where docked toasts stack, so two at once sit one above the other. */
export const ToastDock = createContext<HTMLElement | null>(null);

export interface WindowProps {
  /** The screen (a hub, flow or bare layout). */
  readonly children: ReactNode;
  /** Open sheets and dialogs, bottom to top. Only the top one is interactive. */
  readonly overlays?: readonly ReactNode[];
}

/**
 * The app window. Overlays sit inside it; everything under the top overlay
 * is `inert`, so focus and assistive tech stay in the overlay. The toast dock
 * sits above them all: a note that asks (a file, a link) stays answerable
 * while a dialog is open, and the overlay's focus trap takes it in.
 */
export function Window({ children, overlays = [] }: WindowProps) {
  const top = overlays.length - 1;
  const [dock, setDock] = useState<HTMLElement | null>(null);
  return (
    <div className="p-window">
      <div className="p-window__base" inert={overlays.length > 0}>
        <ToastDock value={dock}>{children}</ToastDock>
      </div>
      {overlays.map((overlay, i) => (
        <div key={i} inert={i < top}>
          {overlay}
        </div>
      ))}
      <div ref={setDock} className="p-toast-dock" />
    </div>
  );
}
