import { createContext, useContext } from 'react';
import type { IconName } from '../Icon/icons';

/** One entry of the hub's top bar. */
export interface HubLink {
  readonly id: string;
  readonly label: string;
  readonly icon: IconName;
  readonly current: boolean;
  readonly onSelect: () => void;
}

/** Where the user is inside a task flow. */
export interface FlowFrame {
  /** The flow's name ("Send"). */
  readonly name: string;
  /** 1-based. */
  readonly step: number;
  readonly total: number;
  /** Close the flow and go home. */
  readonly onClose: () => void;
}

/**
 * Navigation chrome supplied by whoever renders screens (the app shell or
 * the dev gallery). Layouts read it; screens never see routes.
 */
export interface Chrome {
  readonly hubLinks: readonly HubLink[];
  readonly onHome: () => void;
  readonly flow: FlowFrame | null;
}

export const ChromeContext = createContext<Chrome | null>(null);

/** The current chrome. Rendering a layout outside a provider is a programming error. */
export function useChrome(): Chrome {
  const chrome = useContext(ChromeContext);
  if (chrome === null) throw new Error('layouts must be rendered inside a ChromeContext provider');
  return chrome;
}
