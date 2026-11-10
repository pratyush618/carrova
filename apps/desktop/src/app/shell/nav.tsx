import { createContext, useContext } from 'react';
import type { NavEvent, Overlay } from '../nav/navMachine';

/** Navigation as screens use it: raise an event; overlays open and close. */
export interface NavApi {
  readonly go: (event: NavEvent) => void;
  readonly open: (overlay: Overlay) => void;
  readonly close: () => void;
}

export const NavContext = createContext<NavApi | null>(null);

export function useNav(): NavApi {
  const nav = useContext(NavContext);
  if (nav === null) throw new Error('useNav must be used inside the Shell');
  return nav;
}
