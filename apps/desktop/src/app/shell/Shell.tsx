import { useCallback, useEffect, useMemo, useReducer, useRef, type RefObject } from 'react';
import { useTranslation } from 'react-i18next';
import { bundleStatus } from '../../flows/catalog/catalogViews';
import { ChromeContext, Toast, Window } from '../../ui';
import { useControllers } from '../controllers/Controllers';
import type { FlowId } from '../nav/flows';
import { navigate, type Nav, type NavEvent } from '../nav/navMachine';
import { useSession } from '../state/EngineState';
import { buildChrome } from './chrome';
import { useForwardedSends } from './forward';
import { liveView, viewKey } from './liveView';
import { NavContext, type NavApi } from './nav';
import { useAutoOverlays } from './routes/autoOverlays';
import { OverlayRoute } from './routes/OverlayRoute';
import { ViewRoute } from './routes/ViewRoute';

/** Moves focus to the new screen's heading on every navigation (not on first paint). */
function useScreenFocus(key: string, root: RefObject<HTMLDivElement | null>): void {
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    root.current?.querySelector<HTMLElement>('[data-autofocus]')?.focus({ preventScroll: true });
  }, [key, root]);
}

/** The app: navigation state, chrome, the current screen and its overlays. */
export function Shell({ initial }: { readonly initial: Nav }) {
  const { t } = useTranslation();
  const controllers = useControllers();
  const [nav, dispatch] = useReducer(navigate, initial);
  const root = useRef<HTMLDivElement>(null);
  const sending = useSession(controllers.send.state.session);
  const view = liveView(nav.view, controllers, sending);

  const { send, receive } = controllers;
  const leave = useCallback(
    (flow: FlowId) => {
      if (flow === 'send' || flow === 'save') send.abandon();
      if (flow === 'receive' || flow === 'open') receive.abandon();
    },
    [send, receive],
  );
  const go = useCallback(
    (event: NavEvent) => {
      if (event.type === 'closeFlow' && nav.view.kind === 'flow') leave(nav.view.at.flow);
      dispatch(event);
    },
    [leave, nav.view],
  );
  const api = useMemo<NavApi>(
    () => ({
      go,
      open: (overlay) => {
        dispatch({ type: 'openOverlay', overlay });
      },
      close: () => {
        dispatch({ type: 'closeOverlay' });
      },
    }),
    [go],
  );
  const forwarded = useForwardedSends(go, nav.view);
  const recipes = controllers.settings.recipes;
  const chrome = useMemo(
    () => buildChrome(view, t, go, bundleStatus(recipes, t)),
    [view, t, go, recipes],
  );
  useScreenFocus(viewKey(view), root);

  const auto = useAutoOverlays(view);
  const overlays = [
    ...auto,
    ...nav.overlays.map((overlay, i) => (
      <OverlayRoute key={`${String(i)}-${overlay.kind}`} overlay={overlay} />
    )),
  ];
  return (
    <div ref={root} className="p-shell">
      <NavContext value={api}>
        <ChromeContext value={chrome}>
          <Window key={viewKey(view)} overlays={overlays}>
            <ViewRoute view={view} />
            {forwarded.refused && (
              <Toast onDismiss={forwarded.dismiss}>{t('start.forwarded.busy')}</Toast>
            )}
          </Window>
        </ChromeContext>
      </NavContext>
    </div>
  );
}
