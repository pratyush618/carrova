import { flowStart, position, stepIndex, type FlowId, type FlowPosition } from './flows';

/** Places reachable from the hub's top bar. */
export type HubPage = 'home' | 'history' | 'apps' | 'settings';

/** What fills the window. */
export type View =
  | { readonly kind: 'hub'; readonly page: HubPage }
  | { readonly kind: 'flow'; readonly at: FlowPosition };

/** Why an override dialog is open: each names one rule the user may bend. */
export type OverrideKind = 'unmovable' | 'running' | 'space' | 'cross_os' | 'elsewhere';

/** Sheets and dialogs over the view. */
export type Overlay =
  | { readonly kind: 'file-choose' }
  | { readonly kind: 'customize' }
  | { readonly kind: 'unrecognised' }
  | { readonly kind: 'review-details' }
  /** Where cross-system items land (receive review). */
  | { readonly kind: 'mappings' }
  /** What a plugin may do, and whether to allow it for this move. */
  | { readonly kind: 'plugin-consent'; readonly plugin: string }
  | { readonly kind: 'undo-confirm' }
  | { readonly kind: 'update' }
  | { readonly kind: 'report'; readonly id: string }
  | { readonly kind: 'about' }
  /** `subject`: the item key (unmovable, cross_os, elsewhere) or app id (running) the override is about. */
  | { readonly kind: 'override'; readonly reason: OverrideKind; readonly subject: string | null }
  /** Restore an item somewhere else (after the "elsewhere" warning). */
  | { readonly kind: 'remap'; readonly item: string }
  /** Confirm forgetting a session and its undo point. */
  | { readonly kind: 'forget'; readonly id: string };

/** Navigation state: one view, the hub page a flow returns to, and a stack of overlays (top last). */
export interface Nav {
  readonly view: View;
  /** The hub page last shown; leaving a flow lands here. */
  readonly origin: HubPage;
  readonly overlays: readonly Overlay[];
}

/** Every way navigation can change. Screens raise intents; the shell maps them to these. */
export type NavEvent =
  | { readonly type: 'openHub'; readonly page: HubPage }
  | { readonly type: 'startFlow'; readonly flow: FlowId }
  | { readonly type: 'enterFlow'; readonly at: FlowPosition }
  | { readonly type: 'next' }
  | { readonly type: 'back' }
  | { readonly type: 'closeFlow' }
  | { readonly type: 'openOverlay'; readonly overlay: Overlay }
  | { readonly type: 'closeOverlay' };

/** Where the app opens: Home (it does onboarding on a fresh install). */
export const atHome: Nav = { view: { kind: 'hub', page: 'home' }, origin: 'home', overlays: [] };

const show = (nav: Nav, view: View): Nav => ({ ...nav, view, overlays: [] });

const leave = (nav: Nav): Nav => show(nav, { kind: 'hub', page: nav.origin });

/** Moves `delta` steps within the current flow; past either end leaves for the origin page. */
function step(nav: Nav, delta: 1 | -1): Nav {
  if (nav.view.kind !== 'flow') return nav;
  const next = position(nav.view.at.flow, stepIndex(nav.view.at) + delta);
  return next ? show(nav, { kind: 'flow', at: next }) : leave(nav);
}

/** The navigation reducer. Pure; any event that doesn't apply leaves `nav` unchanged. */
export function navigate(nav: Nav, event: NavEvent): Nav {
  switch (event.type) {
    case 'openHub':
      return { view: { kind: 'hub', page: event.page }, origin: event.page, overlays: [] };
    case 'startFlow':
      return show(nav, { kind: 'flow', at: flowStart(event.flow) });
    case 'enterFlow':
      return show(nav, { kind: 'flow', at: event.at });
    case 'next':
      return step(nav, 1);
    case 'back':
      return step(nav, -1);
    case 'closeFlow':
      return nav.view.kind === 'flow' ? leave(nav) : nav;
    case 'openOverlay':
      return { ...nav, overlays: [...nav.overlays, event.overlay] };
    case 'closeOverlay':
      return { ...nav, overlays: nav.overlays.slice(0, -1) };
  }
}
