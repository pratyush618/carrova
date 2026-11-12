import { useCallback, useEffect, useMemo, useReducer, useRef, type RefObject } from 'react';
import { useTranslation } from 'react-i18next';
import { usePort } from '../../engine/port';
import { bundleStatus } from '../../flows/catalog/catalogViews';
import { ChromeContext, Toast, Window } from '../../ui';
import { useControllers } from '../controllers/Controllers';
import type { FlowId } from '../nav/flows';
import { navigate, type Nav, type NavEvent } from '../nav/navMachine';
import { useNearbyRequests, useSession } from '../state/EngineState';
import { buildChrome } from './chrome';
import { useForwardedSends } from './forward';
import { useBell } from './useBell';
import { liveView, viewKey } from './liveView';
import { NavContext, type NavApi } from './nav';
import { RememberedToast, useRememberedNotice } from './RememberedToast';
import { useAutoOverlays } from './routes/autoOverlays';
import { OverlayRoute } from './routes/OverlayRoute';
import { ViewRoute } from './routes/ViewRoute';

/**
 * Moves focus to the new screen's heading on every navigation (not on first
 * paint); a hub page without one falls back to its main region.
 */
function useScreenFocus(key: string, root: RefObject<HTMLDivElement | null>): void {
  const first = useRef(true);
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    const target =
      root.current?.querySelector<HTMLElement>('[data-autofocus]') ??
      root.current?.querySelector<HTMLElement>('[data-autofocus-fallback]');
    target?.focus({ preventScroll: true });
  }, [key, root]);
}

/**
 * The tray's "Nearby" opens the Nearby page, unless a flow is open (a move is
 * never left mid-way). Asked at start too: a window built for it was not
 * listening when the tray asked.
 */
function useTrayNearby(onHub: boolean, dispatch: (event: NavEvent) => void): void {
  const port = usePort();
  const asked = useNearbyRequests();
  const hub = useRef(onHub);
  useEffect(() => {
    hub.current = onHub;
  });
  useEffect(() => {
    void port.takeShowNearby().then(
      (yes) => {
        if (yes && hub.current) dispatch({ type: 'openHub', page: 'nearby' });
      },
      () => undefined,
    );
  }, [port, asked, dispatch]);
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
  useTrayNearby(nav.view.kind === 'hub', dispatch);
  const notice = useRememberedNotice();
  const recipes = controllers.settings.recipes;
  const bell = useBell(go);
  const visibility = controllers.devices.visibility;
  const visible = visibility !== null && visibility.choice !== 'no_one';
  const chrome = useMemo(
    () => buildChrome(view, t, go, bundleStatus(recipes, t), bell, visible),
    [view, t, go, recipes, bell, visible],
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
            {notice.remembered && (
              // Keyed: a device remembered next gets a notice (and Forget) of its own.
              <RememberedToast
                key={notice.remembered.fingerprint}
                remembered={notice.remembered}
                onClose={notice.close}
              />
            )}
          </Window>
        </ChromeContext>
      </NavContext>
    </div>
  );
}
