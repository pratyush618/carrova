import type { TFunction } from 'i18next';
import type { BellModel, Chrome, HubLink, HubStatus, IconName } from '../../ui';
import { STEP_LABELS, stepIndex } from '../nav/flows';
import type { HubPage, NavEvent, View } from '../nav/navMachine';

const HUB: readonly { page: HubPage; icon: IconName }[] = [
  { page: 'home', icon: 'home' },
  { page: 'nearby', icon: 'radar' },
  { page: 'messages', icon: 'msg' },
  { page: 'calls', icon: 'call' },
  { page: 'history', icon: 'history' },
  { page: 'apps', icon: 'apps' },
  { page: 'settings', icon: 'settings' },
];

/**
 * The layouts' chrome for `view`: sidebar links (Nearby with a dot while
 * others can find this computer; the bundle state at its foot, once known)
 * and the bell, or the flow's name and labelled steps.
 */
export function buildChrome(
  view: View,
  t: TFunction,
  dispatch: (event: NavEvent) => void,
  hubStatus: HubStatus | null = null,
  bell?: BellModel,
  visible = false,
  /** A linked phone shares its texts here: Messages shows. */
  messages = false,
  /** A linked phone shares its calls here: Calls shows. */
  calls = false,
): Chrome {
  // A device page sits under Nearby; a phone's calls under Calls, if shown.
  const current =
    view.kind === 'hub'
      ? view.page
      : view.kind === 'flow'
        ? null
        : view.kind === 'calls' && calls
          ? 'calls'
          : 'nearby';
  const shown = (page: HubPage) => (page !== 'messages' || messages) && (page !== 'calls' || calls);
  const hubLinks: HubLink[] = HUB.filter(({ page }) => shown(page)).map(({ page, icon }) => ({
    id: page,
    label: t(`nav.${page}`),
    icon,
    current: page === current,
    ...(page === 'nearby' && visible && { live: true }),
    onSelect: () => {
      dispatch({ type: 'openHub', page });
    },
  }));
  const flow =
    view.kind === 'flow'
      ? {
          name: t(`flow.names.${view.at.flow}`),
          steps: STEP_LABELS[view.at.flow].map((label) => t(`flow.steps.${label}`)),
          step: stepIndex(view.at) + 1,
          onClose: () => {
            dispatch({ type: 'closeFlow' });
          },
        }
      : null;
  return { hubLinks, hubStatus, flow, ...(bell && { bell }) };
}
