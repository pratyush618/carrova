import { createContext, useContext } from 'react';
import type { IconName } from '../Icon/icons';

/** One entry of the hub's sidebar. */
export interface HubLink {
  readonly id: string;
  readonly label: string;
  readonly icon: IconName;
  readonly current: boolean;
  readonly onSelect: () => void;
  /** A small count after the label (moves in History). */
  readonly count?: string;
}

/** The sidebar foot: the recipe bundle's state ("Recipes up to date", "Bundle 2026.11.3 · signed"). */
export interface HubStatus {
  readonly title: string;
  readonly detail: string;
  /** `warning` when the recipes in use are not signed. */
  readonly tone: 'success' | 'warning';
}

/** Where the user is inside a task flow. */
export interface FlowFrame {
  /** The flow's name ("Send"). */
  readonly name: string;
  /** Every step's label, in order ("Pick", "Pair", …). */
  readonly steps: readonly string[];
  /** 1-based position in `steps`. */
  readonly step: number;
  /** Close the flow and go home. */
  readonly onClose: () => void;
}

/**
 * Navigation chrome supplied by whoever renders screens (the app shell or
 * the dev gallery). Layouts read it; screens never see routes.
 */
export interface Chrome {
  readonly hubLinks: readonly HubLink[];
  /** `null` until the bundle state is known. */
  readonly hubStatus: HubStatus | null;
  readonly flow: FlowFrame | null;
}

export const ChromeContext = createContext<Chrome | null>(null);

/** The current chrome. Rendering a layout outside a provider is a programming error. */
export function useChrome(): Chrome {
  const chrome = useContext(ChromeContext);
  if (chrome === null) throw new Error('layouts must be rendered inside a ChromeContext provider');
  return chrome;
}
