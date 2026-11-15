import { useCallback } from 'react';
import { usePort } from '../engine/port';
import { current } from '../flows/remote';
import { useRemote } from '../flows/useRemote';
import { PhoneApp } from '../mobile/PhoneApp';
import { atHome } from './nav/navMachine';
import { Shell } from './shell/Shell';

/**
 * The phone layouts on Android, else the desktop app opening on Home (Home
 * asks the one-time error-report question itself). Nothing shows until the
 * OS is known; if it can't be, the desktop app does.
 */
export function Startup() {
  const port = usePort();
  const about = useRemote(useCallback(() => port.about(), [port]));
  if (about.state.status === 'idle' || about.state.status === 'loading') return null;
  return current(about.state)?.os === 'android' ? <PhoneApp /> : <Shell initial={atHome} />;
}
