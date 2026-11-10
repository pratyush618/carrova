/**
 * The engine port the app uses. Screens and app state talk to the engine
 * only through `CarrovaPort`; this module picks its implementation:
 *
 * - inside the Tauri app (`window.__TAURI_INTERNALS__` present): the bridge;
 * - otherwise, in non-production builds only (Vite dev server, the gallery
 *   and the `--mode e2e` bundle Playwright drives): the in-memory fake,
 *   acting out `?fake=<scenario>` (default `happy`).
 *
 * The fake is loaded with a dynamic `import()` behind a build-time constant,
 * so the production bundle (`pnpm build`, embedded by Tauri) contains none of
 * it. Tests and the gallery import it directly from `./fake`.
 */
import type { CarrovaPort } from './port';
import { createTauriPort } from './tauri';

export type { CarrovaPort, Unsubscribe } from './port';
export { isBridgeError, toBridgeError } from './guards';

/** Whether this build may stand in the fake port for a missing Tauri bridge. */
const FAKE_PORT_ALLOWED = import.meta.env.DEV || import.meta.env.MODE !== 'production';

/** Whether this page runs inside the Tauri app. */
function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** The port for this page. */
export async function createPort(): Promise<CarrovaPort> {
  if (!FAKE_PORT_ALLOWED || inTauri()) {
    return createTauriPort();
  }
  const { createFakePort, scenarioFromSearch } = await import('./fake');
  const search = typeof window === 'undefined' ? '' : window.location.search;
  return createFakePort({ scenario: scenarioFromSearch(search) ?? 'happy' });
}
