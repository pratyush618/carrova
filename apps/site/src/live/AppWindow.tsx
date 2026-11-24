import type { ReactNode } from 'react';
import { buildChrome } from '../../../desktop/src/app/shell/chrome';
import type { View } from '../../../desktop/src/app/nav/navMachine';
import { i18n } from '../../../desktop/src/i18n/i18n';
import { ChromeContext, type HubStatus, Window } from '../../../desktop/src/ui';

const ignore = (): void => undefined;

// What the sidebar foot shows in the gallery too (the real app reads the bundle).
const HUB_STATUS: HubStatus = {
  title: 'Recipes up to date',
  tone: 'success',
  detail: 'Bundle 2026.11.3 · signed',
};

export const flowAt = (flow: 'send' | 'receive', step: string): View =>
  ({ kind: 'flow', at: { flow, step } }) as View;

/** One app window, exactly as the screen gallery frames a screen. */
export function AppWindow({
  view,
  children,
}: {
  readonly view: View;
  readonly children: ReactNode;
}) {
  return (
    <div className="live-window">
      <ChromeContext value={buildChrome(view, i18n.t, ignore, HUB_STATUS)}>
        <Window overlays={[]}>{children}</Window>
      </ChromeContext>
    </div>
  );
}
