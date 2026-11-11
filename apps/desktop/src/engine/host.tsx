import { createContext, useContext, type ReactNode } from 'react';

/**
 * What the desktop offers beyond the engine: native file dialogs and handing
 * a URL or folder to the system. Each capability is optional; screens offer
 * the matching control only when it exists (e.g. a typed path instead of a
 * native dialog in the browser build).
 */
export interface DesktopHost {
  /** Native "save as"; `null` when the user cancels. */
  readonly pickSavePath?: (suggestedName: string) => Promise<string | null>;
  /** Native "open"; `null` when the user cancels. */
  readonly pickOpenPath?: () => Promise<string | null>;
  /** Native multi-select file picker starting in `startIn` (home when `null`); `null` when cancelled. */
  readonly pickFiles?: (startIn: string | null) => Promise<string[] | null>;
  /** Native multi-select folder picker starting in `startIn` (home when `null`); `null` when cancelled. */
  readonly pickFolders?: (startIn: string | null) => Promise<string[] | null>;
  /** Open a web page in the user's browser. */
  readonly openUrl?: (url: string) => Promise<void>;
  /** Show a file or folder in the file manager. */
  readonly revealPath?: (path: string) => Promise<void>;
}

const HostContext = createContext<DesktopHost>({});

export function HostProvider({
  host,
  children,
}: {
  readonly host: DesktopHost;
  readonly children: ReactNode;
}) {
  return <HostContext value={host}>{children}</HostContext>;
}

/** The desktop host (no capabilities outside a provider). */
export function useHost(): DesktopHost {
  return useContext(HostContext);
}
