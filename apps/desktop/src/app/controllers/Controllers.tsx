import { createContext, useContext, useMemo, type ReactNode } from 'react';
import { useHost } from '../../engine/host';
import { usePort } from '../../engine/port';
import type { MoveDefaults } from '../../engine/types';
import { useSaveFlow, type SaveController } from '../../flows/archive/useSaveFlow';
import { useCatalog, type CatalogController } from '../../flows/catalog/useCatalog';
import { useHistory, type HistoryController } from '../../flows/history/useHistory';
import { useReceiveFlow, type ReceiveController } from '../../flows/receive/useReceiveFlow';
import { useSendFlow, type SendController } from '../../flows/send/useSendFlow';
import { useAppUpdate, type AppUpdateController } from '../../flows/settings/useAppUpdate';
import { useErrorReport, type ErrorReportController } from '../../flows/settings/useErrorReport';
import { useSettings, type SettingsController } from '../../flows/settings/useSettings';
import { useUndo, type UndoController } from '../../flows/undo/useUndo';

/** One controller per flow or page. Screens get them here; none talks to the port itself. */
export interface Controllers {
  readonly send: SendController;
  readonly save: SaveController;
  readonly receive: ReceiveController;
  readonly history: HistoryController;
  readonly catalog: CatalogController;
  readonly settings: SettingsController;
  readonly update: AppUpdateController;
  readonly report: ErrorReportController;
  readonly undo: UndoController;
}

/** Until Settings load: the engine's own suggestions. */
const DEFAULT_MOVE_DEFAULTS: MoveDefaults = {
  conflict_rule: 'replace',
  retention: 'until_dismissed',
  include_caches: false,
};

const ControllersContext = createContext<Controllers | null>(null);

/** Creates every controller once, over the port and host from context. */
export function ControllersProvider({ children }: { readonly children: ReactNode }) {
  const port = usePort();
  const host = useHost();
  const update = useAppUpdate(port);
  const settings = useSettings(port, update.check);
  const defaults = settings.settings?.move_defaults ?? DEFAULT_MOVE_DEFAULTS;
  const send = useSendFlow(port, defaults.include_caches);
  const save = useSaveFlow(port, send, host);
  const receive = useReceiveFlow(port, host, defaults);
  const history = useHistory(port);
  const catalog = useCatalog(port);
  const report = useErrorReport(port, host, settings.reloadReports);
  const undo = useUndo(port, history.reload);
  const value = useMemo(
    () => ({ send, save, receive, history, catalog, settings, update, report, undo }),
    [send, save, receive, history, catalog, settings, update, report, undo],
  );
  return <ControllersContext value={value}>{children}</ControllersContext>;
}

export function useControllers(): Controllers {
  const controllers = useContext(ControllersContext);
  if (controllers === null)
    throw new Error('useControllers must be used inside ControllersProvider');
  return controllers;
}
