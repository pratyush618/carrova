import type { TFunction } from 'i18next';
import type { Chrome, HubLink, IconName } from '../../ui';
import { flowLength, stepIndex } from '../nav/flows';
import type { HubPage, NavEvent, View } from '../nav/navMachine';

const HUB: readonly { page: HubPage; icon: IconName }[] = [
  { page: 'home', icon: 'swap' },
  { page: 'history', icon: 'history' },
  { page: 'apps', icon: 'apps' },
  { page: 'settings', icon: 'settings' },
];

/** The layouts' chrome for `view`: top-bar links, or the flow's name and step. */
export function buildChrome(view: View, t: TFunction, dispatch: (event: NavEvent) => void): Chrome {
  const current = view.kind === 'hub' ? view.page : null;
  const hubLinks: HubLink[] = HUB.map(({ page, icon }) => ({
    id: page,
    label: t(`nav.${page}`),
    icon,
    current: page === current,
    onSelect: () => {
      dispatch({ type: 'openHub', page });
    },
  }));
  const flow =
    view.kind === 'flow'
      ? {
          name: t(`flow.names.${view.at.flow}`),
          step: stepIndex(view.at) + 1,
          total: flowLength(view.at.flow),
          onClose: () => {
            dispatch({ type: 'closeFlow' });
          },
        }
      : null;
  return {
    hubLinks,
    flow,
    onHome: () => {
      dispatch({ type: 'openHub', page: 'home' });
    },
  };
}
