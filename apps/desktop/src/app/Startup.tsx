import { atHome } from './nav/navMachine';
import { Shell } from './shell/Shell';

/** The app opens on Home; Home asks the one-time error-report question itself. */
export function Startup() {
  return <Shell initial={atHome} />;
}
