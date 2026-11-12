import type { TFunction } from 'i18next';
import type { BellModel, Chrome, HubLink, HubStatus, IconName } from '../../ui';
import { STEP_LABELS, stepIndex } from '../nav/flows';
import type { HubPage, NavEvent, View } from '../nav/navMachine';

const HUB: readonly { page: HubPage; icon: IconName }[] = [
  { page: 'home', icon: 'home' },
  { page: 'nearby', icon: 'radar' },
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
): Chrome {
  const current = view.kind === 'hub' ? view.page : null;
  const hubLinks: HubLink[] = HUB.map(({ page, icon }) => ({
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
