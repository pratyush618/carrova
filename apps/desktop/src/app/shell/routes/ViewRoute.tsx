import type { SettingsSection } from '../../../screens/settings/sections';
import type { HubPage, View } from '../../nav/navMachine';
import { AppsRoute, HistoryRoute, HomeRoute, SettingsRoute } from './HubRoutes';
import { OpenRoute, ReceiveRoute } from './ReceiveRoutes';
import { SaveRoute, SendRoute } from './SendRoutes';
import { UndoRoute } from './UndoRoute';

function HubRoute({
  page,
  section = 'general',
}: {
  readonly page: HubPage;
  readonly section?: SettingsSection | undefined;
}) {
  switch (page) {
    case 'home':
      return <HomeRoute />;
    case 'history':
      return <HistoryRoute />;
    case 'apps':
      return <AppsRoute />;
    case 'settings':
      return <SettingsRoute section={section} />;
  }
}

/** The screen for `view`. Exhaustive over every page and flow position. */
export function ViewRoute({ view }: { readonly view: View }) {
  switch (view.kind) {
    case 'hub':
      return <HubRoute page={view.page} section={view.section} />;
    case 'flow': {
      const { at } = view;
      switch (at.flow) {
        case 'send':
          return <SendRoute step={at.step} />;
        case 'save':
          return <SaveRoute step={at.step} />;
        case 'receive':
          return <ReceiveRoute step={at.step} />;
        case 'open':
          return <OpenRoute step={at.step} />;
        case 'undo':
          return at.step === 'result' ? <UndoRoute /> : <HomeRoute />;
      }
    }
  }
}
