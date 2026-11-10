import { lazy, Suspense, type ReactNode } from 'react';
import { HostProvider, type DesktopHost } from '../engine/host';
import { PortProvider } from '../engine/port';
import type { CarrovaPort } from '../engine/types';
import { wantsGallery } from '../gallery/useHashTarget';
import { PreferencesProvider } from '../preferences/preferences';
import { ControllersProvider } from './controllers/Controllers';
import { SettingsSync } from './controllers/SettingsSync';
import { Startup } from './Startup';
import { EngineStateProvider } from './state/EngineState';

/**
 * The dev screen gallery exists only in dev servers and `--mode gallery`
 * builds (Playwright); production builds drop it entirely.
 */
const GALLERY_ENABLED = import.meta.env.DEV || import.meta.env.MODE === 'gallery';

const Gallery = GALLERY_ENABLED
  ? lazy(() => import('../gallery/Gallery').then((m) => ({ default: m.Gallery })))
  : null;

/** The live app: port and host in context, engine events folded, every controller created. */
function Live({
  port,
  host,
  children,
}: {
  readonly port: CarrovaPort;
  readonly host: DesktopHost;
  readonly children: ReactNode;
}) {
  return (
    <PortProvider port={port}>
      <HostProvider host={host}>
        <EngineStateProvider>
          <ControllersProvider>
            <SettingsSync />
            {children}
          </ControllersProvider>
        </EngineStateProvider>
      </HostProvider>
    </PortProvider>
  );
}

/** Application root: preferences, then the gallery (dev) or the live app. */
export function App({ port, host }: { readonly port: CarrovaPort; readonly host: DesktopHost }) {
  return (
    <PreferencesProvider>
      {Gallery && wantsGallery() ? (
        <Suspense fallback={null}>
          <Gallery />
        </Suspense>
      ) : (
        <Live port={port} host={host}>
          <Startup />
        </Live>
      )}
    </PreferencesProvider>
  );
}
